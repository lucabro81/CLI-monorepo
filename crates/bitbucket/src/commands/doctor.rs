//! Handler for the `doctor` command.
//!
//! Runs four sequential checks for one identity (the OAuth app, or the person
//! with `--user <id>`) and returns a structured JSON report:
//!
//! 1. `app_config` — verifies that `app.json` exists at the expected path and
//!    has a valid consumer section for the identity.
//! 2. `credentials` — verifies that the identity's credentials file
//!    (`credentials-service.json` / `users/<id>/credentials.json`) exists and holds a
//!    non-expired token, and reports its `identity` (`"user"` after
//!    `auth login --user <id>`, `"app"` after `auth login`). If the token is expired,
//!    a renewal is attempted through the matching grant (`refresh_token` or
//!    `client_credentials`, under the same lock as every command's) and the
//!    result (success or failure) is reported.
//! 3. `api` — makes a live call to `/2.0/user` to confirm the Bitbucket API
//!    is reachable with the current token.
//! 4. `permissions` — lists the OAuth scopes granted to the consumer, taken
//!    from `credentials.scopes` (captured from the token response at login
//!    time, no extra API call needed). `status` is `"ok"` if any scopes were
//!    granted at all, `"error"` if the list is empty (nothing will work).
//!    Purely informational beyond that — which scopes a given command needs
//!    is documented in this crate's CLAUDE.md, not enforced here.
//!
//! Checks cascade: if `app_config` fails, the remaining checks are marked
//! `skipped` (no credentials to load). If `credentials` fails, `api` and
//! `permissions` are skipped (no token to use). `permissions` does not depend
//! on `api` and runs whenever `credentials` succeeds.
//!
//! A last, informational `pending_login` check (outside the cascade and
//! `all_ok`) reports whether the selected person's two-step
//! `auth login --user <id> --remote` is waiting for its code (always `none` for
//! the OAuth app). Another, `identities` (also informational, no network),
//! reports the identity checked, whether the OAuth app is logged in, the ids of
//! the people logged in, and credentials files of earlier layouts still lying
//! around.
//!
//! The function never returns `Err` for check failures — all outcomes are
//! captured in the JSON report. The caller decides whether to exit non-zero
//! based on the returned `bool` flag. This module is also called by `init`
//! as a final verification step after onboarding.

use serde_json::{json, Value};

use crate::auth::{self, AppConfig, Identity, OAuthConfig};
use crate::client::{BitbucketClient, ClientError};
use crate::context::{app_config_error, config_dir, login_command, oauth_section, rejected_token_renewer};
use crate::error::CliError;

/// Runs all doctor checks for `identity`. Returns `(report, all_ok)`.
///
/// `report` is a JSON object with one key per check. `all_ok` is `true` only
/// if every check has `status: "ok"`.
pub fn run_doctor(identity: &Identity) -> Result<(Value, bool), CliError> {
    let config_dir = config_dir()?;

    let (app_check, oauth_config) = check_app_config(&config_dir, identity);
    let app_passed = app_check["status"] == "ok";

    let (mut creds_check, credentials) = match oauth_config {
        Some(ref config) if app_passed => check_credentials(config, &config_dir, identity),
        _ => (skipped("app_config check failed"), None),
    };
    let mut creds_passed = creds_check["status"] == "ok";

    let connectivity_check = match (&oauth_config, &credentials) {
        (Some(config), Some(creds)) if creds_passed => {
            let path = auth::credentials_path(&config_dir, identity);
            let client = BitbucketClient::new(creds)
                .with_renewer(rejected_token_renewer(config.clone(), path.clone(), identity.clone()));
            match api_or_credentials_error(&client, &path) {
                Ok(check) => check,
                Err(check) => {
                    creds_check = check;
                    creds_passed = false;
                    skipped("credentials check failed")
                }
            }
        }
        _ => skipped("credentials check failed"),
    };
    let connectivity_passed = connectivity_check["status"] == "ok";

    let permissions_check = match credentials {
        Some(ref creds) if creds_passed => check_permissions(creds),
        _ => skipped("credentials check failed"),
    };
    let permissions_passed = permissions_check["status"] == "ok";

    let all_ok = app_passed && creds_passed && connectivity_passed && permissions_passed;

    let report = json!({
        "app_config": app_check,
        "credentials": creds_check,
        "api": connectivity_check,
        "permissions": permissions_check,
        "pending_login": check_pending_login(&config_dir, identity, auth::now_unix()),
        "identities": check_identities(&config_dir, identity),
    });

    Ok((report, all_ok))
}

fn check_app_config(config_dir: &std::path::Path, identity: &Identity) -> (Value, Option<OAuthConfig>) {
    let path = auth::app_config_path(config_dir);
    let path_str = path.display().to_string();

    let section = AppConfig::load(&path)
        .map_err(|e| app_config_error(e, &path))
        .and_then(|app| oauth_section(app, identity, &path));
    match section {
        Ok(config) => (json!({"status": "ok", "path": path_str}), Some(config)),
        Err(e) => (
            json!({"status": "error", "path": path_str, "message": e.to_string()}),
            None,
        ),
    }
}

fn check_credentials(
    oauth_config: &OAuthConfig,
    config_dir: &std::path::Path,
    selected: &Identity,
) -> (Value, Option<auth::Credentials>) {
    let path = auth::credentials_path(config_dir, selected);
    let path_str = path.display().to_string();
    let login = login_command(selected);

    let Ok(raw) = std::fs::read_to_string(&path) else {
        return (
            json!({
                "status": "error",
                "path": path_str,
                "message": format!("credentials file not found at {path_str}. Run: {login}")
            }),
            None,
        );
    };

    let Ok(credentials) = serde_json::from_str::<auth::Credentials>(&raw) else {
        return (
            json!({
                "status": "error",
                "path": path_str,
                "message": format!("credentials file is malformed. Run: {login}")
            }),
            None,
        );
    };

    if let Err(e) = auth::check_identity(&credentials, selected) {
        return (
            json!({"status": "error", "path": path_str, "message": format!("{e}. Run: {login}")}),
            None,
        );
    }

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    let identity = identity(&credentials);

    if now >= credentials.expires_at {
        // load_credentials renews under the per-file lock, like every command.
        return match auth::load_credentials(oauth_config, &path, selected) {
            Ok(renewed) => (
                json!({
                    "status": "ok",
                    "path": path_str,
                    "identity": identity,
                    "expires_at": renewed.expires_at,
                    "note": "token was expired and has been renewed"
                }),
                Some(renewed),
            ),
            Err(e) => (
                json!({
                    "status": "error",
                    "path": path_str,
                    "identity": identity,
                    "message": format!("token expired and renewal failed: {e}. Run: {login}")
                }),
                None,
            ),
        };
    }

    (
        json!({"status": "ok", "path": path_str, "identity": identity, "expires_at": credentials.expires_at}),
        Some(credentials),
    )
}

/// Which identity `credentials` act as: `"user"` for `auth login --user`
/// (has a refresh token), `"app"` for `client_credentials`.
fn identity(credentials: &auth::Credentials) -> &'static str {
    if credentials.refresh_token.is_some() { "user" } else { "app" }
}

/// The `api` check; `Err` is the `credentials` check to report instead when
/// the API answered 401 and the token could not be renewed (issue #240: a
/// token revoked before it expired is a credentials problem, not an API one).
pub(crate) fn api_or_credentials_error(client: &BitbucketClient, credentials_path: &std::path::Path) -> Result<Value, Value> {
    check_api(client).map_err(|renewal| {
        json!({
            "status": "error",
            "path": credentials_path.display().to_string(),
            "message": renewal.to_string(),
        })
    })
}

/// The `api` check, or the error of a renewal after a 401.
fn check_api(client: &BitbucketClient) -> Result<Value, CliError> {
    Ok(match client.get_current_user() {
        Ok(user) => {
            let username = user["username"].as_str().unwrap_or("unknown").to_string();
            let account_type = user["type"].as_str().unwrap_or("unknown").to_string();
            json!({"status": "ok", "username": username, "type": account_type})
        }
        Err(ClientError::Renewal(e)) => return Err(*e),
        Err(e) => json!({"status": "error", "message": e.to_string()}),
    })
}

/// Lists the OAuth scopes granted to the consumer, from `credentials.scopes`
/// (no API call). `status` is `"error"` only if the list is empty — an empty
/// scope list means no command can do anything. Otherwise purely informational.
fn check_permissions(credentials: &auth::Credentials) -> Value {
    let status = if credentials.scopes.is_empty() { "error" } else { "ok" };

    json!({"status": status, "granted_scopes": credentials.scopes})
}

/// Informational: the selected person's two-step (`--remote`) login waiting
/// for its code. Never affects `all_ok` (an idle CLI has none, and that is
/// fine); the OAuth app never has one.
pub(crate) fn check_pending_login(config_dir: &std::path::Path, identity: &Identity, now: u64) -> Value {
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
            "message": format!("{e}. Start a new remote login with: bitbucket auth login --user {id} --remote"),
        }),
    }
}

/// Informational, no network: which identity this report checks, whether
/// the OAuth app is logged in, the ids of the people logged in, and
/// credentials files of earlier layouts (no longer read) still there to be deleted.
pub(crate) fn check_identities(config_dir: &std::path::Path, selected: &Identity) -> Value {
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

#[cfg(test)]
#[path = "../tests/commands/doctor_tests.rs"]
mod tests;
