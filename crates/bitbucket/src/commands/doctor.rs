//! Handler for the `doctor` command.
//!
//! Runs four sequential checks and returns a structured JSON report:
//!
//! 1. `app_config` — verifies that `app.json` exists at the expected path and
//!    contains valid OAuth consumer credentials.
//! 2. `credentials` — verifies that `credentials.json` exists and holds a
//!    non-expired token. If the token is expired, a renewal is attempted via
//!    `client_credentials` and the result (success or failure) is reported.
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
//! The function never returns `Err` for check failures — all outcomes are
//! captured in the JSON report. The caller decides whether to exit non-zero
//! based on the returned `bool` flag. This module is also called by `init`
//! as a final verification step after onboarding.

use serde_json::{json, Value};

use crate::auth::{self, OAuthConfig};
use crate::client::BitbucketClient;
use crate::context::config_dir;
use crate::error::CliError;

/// Runs all doctor checks. Returns `(report, all_ok)`.
///
/// `report` is a JSON object with one key per check. `all_ok` is `true` only
/// if every check has `status: "ok"`.
pub fn run_doctor() -> Result<(Value, bool), CliError> {
    let config_dir = config_dir()?;

    let (app_check, oauth_config) = check_app_config(&config_dir);
    let app_passed = app_check["status"] == "ok";

    let (creds_check, credentials) = match oauth_config {
        Some(ref config) if app_passed => check_credentials(config, &config_dir),
        _ => (skipped("app_config check failed"), None),
    };
    let creds_passed = creds_check["status"] == "ok";

    let connectivity_check = match credentials {
        Some(ref creds) if creds_passed => check_api(creds),
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
    });

    Ok((report, all_ok))
}

fn check_app_config(config_dir: &std::path::Path) -> (Value, Option<OAuthConfig>) {
    let path = auth::app_config_path(config_dir);
    let path_str = path.display().to_string();

    match OAuthConfig::load(&path) {
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
) -> (Value, Option<auth::Credentials>) {
    let path = auth::credentials_path(config_dir);
    let path_str = path.display().to_string();

    let Ok(raw) = std::fs::read_to_string(&path) else {
        return (
            json!({
                "status": "error",
                "path": path_str,
                "message": format!("credentials file not found at {path_str}. Run: bitbucket auth login")
            }),
            None,
        );
    };

    let Ok(credentials) = serde_json::from_str::<auth::Credentials>(&raw) else {
        return (
            json!({
                "status": "error",
                "path": path_str,
                "message": "credentials file is malformed. Run: bitbucket auth login"
            }),
            None,
        );
    };

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    if now >= credentials.expires_at {
        return match auth::login_client_credentials(oauth_config) {
            Ok(renewed) => {
                let _ = auth::save_credentials(&path, &renewed);
                (
                    json!({
                        "status": "ok",
                        "path": path_str,
                        "expires_at": renewed.expires_at,
                        "note": "token was expired and has been renewed"
                    }),
                    Some(renewed),
                )
            }
            Err(e) => (
                json!({
                    "status": "error",
                    "path": path_str,
                    "message": format!("token expired and renewal failed: {e}. Run: bitbucket auth login")
                }),
                None,
            ),
        };
    }

    (
        json!({"status": "ok", "path": path_str, "expires_at": credentials.expires_at}),
        Some(credentials),
    )
}

fn check_api(credentials: &auth::Credentials) -> Value {
    let client = BitbucketClient::new(credentials);
    match client.get_current_user() {
        Ok(user) => {
            let username = user["username"].as_str().unwrap_or("unknown").to_string();
            let account_type = user["type"].as_str().unwrap_or("unknown").to_string();
            json!({"status": "ok", "username": username, "type": account_type})
        }
        Err(e) => json!({"status": "error", "message": e.to_string()}),
    }
}

/// Lists the OAuth scopes granted to the consumer, from `credentials.scopes`
/// (no API call). `status` is `"error"` only if the list is empty — an empty
/// scope list means no command can do anything. Otherwise purely informational.
fn check_permissions(credentials: &auth::Credentials) -> Value {
    let status = if credentials.scopes.is_empty() { "error" } else { "ok" };

    json!({"status": status, "granted_scopes": credentials.scopes})
}

fn skipped(reason: &str) -> Value {
    json!({"status": "skipped", "reason": reason})
}

#[cfg(test)]
#[path = "../tests/commands/doctor_tests.rs"]
mod tests;
