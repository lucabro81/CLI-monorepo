//! Handler for the `doctor` command.
//!
//! Runs sequential checks and returns a structured JSON report:
//!
//! 1. `app_config` — `app.json` exists and is valid; reports the instance URL and
//!    whether the service user key / Native app client id are configured.
//! 2. `credentials` — `credentials.json` holds a usable token (renewed first if
//!    expiring); reports the identity kind (`service_user` / `user`) and expiry.
//! 3. `api` — live `GET /auth/v1/users/me`: user id, name, type, organization.
//! 4. `memberships` — the identity's administrator roles per instance /
//!    organization / project / project grant. ZITADEL authorizes by these roles,
//!    so they decide which commands will succeed; none at all is an error.
//!
//! 5. `pending_login` — informational, outside the cascade and `all_ok`: whether a
//!    two-step `auth login --user --remote` is waiting for its code, and until when.
//!
//! Checks cascade: a failed check marks every later check `skipped`. Failures
//! never surface as `Err` — they are captured in the report, and the caller
//! exits non-zero based on the returned `all_ok`. Also called by `init`.

use std::path::Path;

use serde_json::{Value, json};

use crate::auth::{self, AppConfig, Credentials, LoginError};
use crate::client::ZitadelClient;
use crate::context::{client_error_to_cli, config_dir};
use crate::error::CliError;

pub fn run_doctor() -> Result<(Value, bool), CliError> {
    Ok(run_doctor_in(&config_dir()?))
}

/// Runs every check against `config_dir`. Returns `(report, all_ok)`.
pub(crate) fn run_doctor_in(config_dir: &Path) -> (Value, bool) {
    let (app_check, config) = check_app_config(config_dir);
    let (creds_check, client) = match &config {
        Some(config) => check_credentials(config, config_dir),
        None => (skipped("app_config check failed"), None),
    };
    let identity_check = match &client {
        Some(client) => check_api(client),
        None => skipped("credentials check failed"),
    };
    let memberships_check = match &client {
        Some(client) if identity_check["status"] == "ok" => check_memberships(client),
        _ => skipped("api check failed"),
    };

    let all_ok = [&app_check, &creds_check, &identity_check, &memberships_check]
        .iter()
        .all(|check| check["status"] == "ok");

    let report = json!({
        "app_config": app_check,
        "credentials": creds_check,
        "api": identity_check,
        "memberships": memberships_check,
        "pending_login": check_pending_login(config_dir, auth::now_unix()),
    });
    (report, all_ok)
}

/// Informational: a two-step (`--remote`) login waiting for its code. Never
/// affects `all_ok` (an idle CLI has none, and that is fine).
pub(crate) fn check_pending_login(config_dir: &Path, now: u64) -> Value {
    match oauth_user_login::pending_login_status(&auth::pending_login_path(config_dir), now) {
        Ok(None) => json!({"status": "none"}),
        Ok(Some(status)) => json!({
            "status": "pending",
            "expires_at": oauth_user_login::rfc3339_utc(status.expires_at),
            "expired": status.expired,
        }),
        Err(e) => json!({
            "status": "error",
            "message": format!(
                "{e}. Start a new remote login with: zitadel auth login --user --remote --redirect-uri <redirect-uri>"
            ),
        }),
    }
}

fn skipped(reason: &str) -> Value {
    json!({"status": "skipped", "reason": reason})
}

fn error(path: Option<&Path>, message: &str) -> Value {
    let mut check = json!({"status": "error", "message": message});
    if let Some(path) = path {
        check["path"] = json!(path.display().to_string());
    }
    check
}

fn check_app_config(config_dir: &Path) -> (Value, Option<AppConfig>) {
    let path = auth::app_config_path(config_dir);
    match AppConfig::load(&path) {
        Ok(config) => (
            json!({
                "status": "ok",
                "path": path.display().to_string(),
                "instance_url": config.instance_url,
                "service_user_configured": config.service_user.is_some(),
                "native_app_configured": config.client_id.is_some(),
            }),
            Some(config),
        ),
        Err(e) => (
            error(
                Some(&path),
                &format!(
                    "{e}. Run: zitadel init --instance-url https://<instance>.zitadel.cloud --key-file <path-to-key.json>"
                ),
            ),
            None,
        ),
    }
}

fn check_credentials(config: &AppConfig, config_dir: &Path) -> (Value, Option<ZitadelClient>) {
    let path = auth::credentials_path(config_dir);
    match auth::load_credentials(config, &path) {
        Ok(credentials) => (
            json!({
                "status": "ok",
                "path": path.display().to_string(),
                "identity": identity_kind(&credentials),
                "expires_at": credentials.expires_at,
            }),
            Some(ZitadelClient::new(&config.instance_url, &credentials)),
        ),
        Err(LoginError::Io(e)) if e.kind() == std::io::ErrorKind::NotFound => (
            error(Some(&path), "no stored credentials. Run: zitadel auth login"),
            None,
        ),
        Err(e) => (
            error(Some(&path), &format!("{e}. Run: zitadel auth login")),
            None,
        ),
    }
}

fn identity_kind(credentials: &Credentials) -> &'static str {
    if credentials.refresh_token.is_some() { "user" } else { "service_user" }
}

fn check_api(client: &ZitadelClient) -> Value {
    match client.get_current_user() {
        Ok(me) => {
            let user = &me["user"];
            let kind = if user.get("machine").is_some() {
                "machine"
            } else if user.get("human").is_some() {
                "human"
            } else {
                "unknown"
            };
            json!({
                "status": "ok",
                "user_id": user["id"],
                "user_name": user["userName"],
                "type": kind,
                "organization_id": user["details"]["resourceOwner"],
            })
        }
        Err(e) => error(None, &client_error_to_cli(e).to_string()),
    }
}

fn check_memberships(client: &ZitadelClient) -> Value {
    match client.list_my_memberships() {
        Ok(raw) => {
            let memberships = summarize_memberships(&raw);
            if memberships.is_empty() {
                error(
                    None,
                    "the identity has no administrator role, so every management command will be \
                    denied (403). In the ZITADEL console grant it one, e.g. ORG_OWNER on an \
                    organization (Organization > Administrators) or IAM_OWNER on the instance \
                    (Default settings > Administrators).",
                )
            } else {
                json!({"status": "ok", "memberships": memberships})
            }
        }
        Err(e) => error(None, &client_error_to_cli(e).to_string()),
    }
}

/// Flattens ZITADEL's membership search result into
/// `{level, id, display_name, roles}` objects, one per membership.
pub(crate) fn summarize_memberships(raw: &Value) -> Vec<Value> {
    let Some(result) = raw["result"].as_array() else {
        return vec![];
    };
    result
        .iter()
        .map(|m| {
            let (level, id) = if let Some(id) = m.get("projectGrantId") {
                ("project_grant", id.clone())
            } else if let Some(id) = m.get("projectId") {
                ("project", id.clone())
            } else if let Some(id) = m.get("orgId") {
                ("organization", id.clone())
            } else {
                ("instance", Value::Null)
            };
            json!({
                "level": level,
                "id": id,
                "display_name": m["displayName"],
                "roles": m.get("roles").cloned().unwrap_or_else(|| json!([])),
            })
        })
        .collect()
}

#[cfg(test)]
#[path = "../tests/commands/doctor_tests.rs"]
mod tests;
