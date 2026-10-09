//! Handler for the `doctor` command.
//!
//! Runs sequential checks and returns a structured JSON report:
//!
//! 1. `app_config` — `app.json` exists and is valid; reports the instance URL and
//!    whether the service user key / Native app client id are configured.
//! 2. `credentials` — the selected identity's credentials file
//!    (`credentials-service.json`, or `users/<id>/credentials.json` with
//!    `--user <id>`) holds a usable token (renewed first if expiring, under the
//!    same lock as every command's); reports the identity kind
//!    (`service_user` / `user`) and expiry. A token the `api` call gets a 401
//!    for is renewed and the call repeated, as every command does; if that
//!    renewal fails, this check reports it (issue #240: a revoked token).
//! 3. `api` — live `GET /auth/v1/users/me`: user id, name, type, organization.
//! 4. `memberships` — the identity's administrator roles per instance /
//!    organization / project / project grant. ZITADEL authorizes by these roles,
//!    so they decide which commands will succeed; none at all is an error.
//!
//! 5. `pending_login` — informational, outside the cascade and `all_ok`: whether
//!    the selected person's two-step `auth login --user <id> --remote` is waiting
//!    for its code, and until when (always `none` for the service user).
//! 6. `identities` — informational, outside the cascade and `all_ok`, no network:
//!    the identity checked, whether the service user is logged in, the ids of the
//!    people logged in, and credentials files of earlier layouts still lying around.
//!
//! Checks cascade: a failed check marks every later check `skipped`. Failures
//! never surface as `Err` — they are captured in the report, and the caller
//! exits non-zero based on the returned `all_ok`. Also called by `init`.

use std::path::Path;

use serde_json::{Value, json};

use crate::auth::{self, AppConfig, Credentials, Identity, LoginError};
use crate::client::{ClientError, ZitadelClient};
use crate::context::{client_error_to_cli, config_dir, login_command, rejected_token_renewer};
use crate::error::CliError;

pub fn run_doctor(identity: &Identity) -> Result<(Value, bool), CliError> {
    Ok(run_doctor_in(&config_dir()?, identity))
}

/// Runs every check against `config_dir` for `identity`. Returns `(report, all_ok)`.
pub(crate) fn run_doctor_in(config_dir: &Path, identity: &Identity) -> (Value, bool) {
    let (app_check, config) = check_app_config(config_dir);
    let (mut creds_check, client) = match &config {
        Some(config) => check_credentials(config, config_dir, identity),
        None => (skipped("app_config check failed"), None),
    };
    let identity_check = match &client {
        Some(client) => match check_api(client) {
            Ok(check) => check,
            // The token was rejected and could not be renewed (issue #240):
            // a credentials problem, not an API one.
            Err(renewal) => {
                creds_check = error(
                    Some(&auth::credentials_path(config_dir, identity)),
                    &renewal.to_string(),
                );
                skipped("credentials check failed")
            }
        },
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
        "pending_login": check_pending_login(config_dir, identity, auth::now_unix()),
        "identities": check_identities(config_dir, identity),
    });
    (report, all_ok)
}

/// Informational: the selected person's two-step (`--remote`) login waiting
/// for its code. Never affects `all_ok` (an idle CLI has none, and that is
/// fine); the service user never has one.
pub(crate) fn check_pending_login(config_dir: &Path, identity: &Identity, now: u64) -> Value {
    let Identity::User(id) = identity else {
        return json!({"status": "none"});
    };
    match oauth_user_login::pending_login_status(&auth::pending_login_path(config_dir, id), now) {
        Ok(None) => json!({"status": "none"}),
        Ok(Some(status)) => json!({
            "status": "pending",
            "expires_at": oauth_user_login::rfc3339_utc(status.expires_at),
            "expired": status.expired,
        }),
        Err(e) => json!({
            "status": "error",
            "message": format!(
                "{e}. Start a new remote login with: zitadel auth login --user {id} --remote --redirect-uri <redirect-uri>"
            ),
        }),
    }
}

/// Informational, no network: which identity this report checks, whether
/// the service user is logged in, the ids of the people logged in, and
/// credentials files of earlier layouts (no longer read) still there to be deleted.
pub(crate) fn check_identities(config_dir: &Path, selected: &Identity) -> Value {
    let service = if auth::credentials_path(config_dir, &Identity::Service).exists() {
        "present"
    } else {
        "missing"
    };
    // An unreadable users/ folder lists nobody; the selected identity's own
    // checks above report any real problem.
    let users: Vec<String> = auth::list_users(config_dir)
        .unwrap_or_default()
        .iter()
        .map(ToString::to_string)
        .collect();
    json!({
        "selected": selected.label(),
        "service": service,
        "users": users,
        "legacy_credentials_files": auth::legacy_credentials_files(config_dir),
    })
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

pub(crate) fn check_app_config(config_dir: &Path) -> (Value, Option<AppConfig>) {
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

fn check_credentials(config: &AppConfig, config_dir: &Path, identity: &Identity) -> (Value, Option<ZitadelClient>) {
    let path = auth::credentials_path(config_dir, identity);
    let login = login_command(identity);
    match auth::load_credentials(config, &path, identity) {
        Ok(credentials) => (
            json!({
                "status": "ok",
                "path": path.display().to_string(),
                "identity": identity_kind(&credentials),
                "expires_at": credentials.expires_at,
            }),
            Some(
                ZitadelClient::new(&config.instance_url, &credentials)
                    .with_renewer(rejected_token_renewer(config.clone(), path.clone(), identity.clone())),
            ),
        ),
        Err(LoginError::Io(e)) if e.kind() == std::io::ErrorKind::NotFound => (
            error(Some(&path), &format!("no stored credentials. Run: {login}")),
            None,
        ),
        Err(e) => (
            error(Some(&path), &format!("{e}. Run: {login}")),
            None,
        ),
    }
}

fn identity_kind(credentials: &Credentials) -> &'static str {
    if credentials.refresh_token.is_some() { "user" } else { "service_user" }
}

/// The `api` check, or the error of a renewal after a 401 (for `credentials`).
fn check_api(client: &ZitadelClient) -> Result<Value, CliError> {
    Ok(match client.get_current_user() {
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
        Err(ClientError::Renewal(e)) => return Err(*e),
        Err(e) => error(None, &client_error_to_cli(e).to_string()),
    })
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
