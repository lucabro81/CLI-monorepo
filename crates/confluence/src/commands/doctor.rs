//! Handler for the `doctor` command.
//!
//! Runs four sequential checks and returns a structured JSON report:
//!
//! 1. `app_config` — verifies that `app.json` exists at the expected path and
//!    contains valid OAuth credentials.
//! 2. `credentials` — verifies that `credentials.json` exists and holds a
//!    non-expired token. If the token is expired, a renewal is attempted and
//!    the result (success or failure) is reported transparently.
//! 3. `api` — makes a live call to `/wiki/rest/api/user/current` to confirm
//!    the Confluence API is reachable with the current token.
//! 4. `oauth_scopes` — lists the OAuth scopes granted to the token (the app
//!    identity layer), via the accessible-resources endpoint.
//!
//! Unlike `jira`'s doctor, there is no per-space permission-scheme layer yet
//! (Confluence's space-permission model is a separate concern from
//! account-level OAuth scopes) — add one here once a concrete command needs
//! it, per root CLAUDE.md's incremental approach.
//!
//! Checks cascade: if `app_config` fails, the remaining checks are marked
//! `skipped` (no credentials to load). If `credentials` fails, `api` and
//! `oauth_scopes` are skipped (no token to use).
//!
//! The function never returns `Err` for check failures — all outcomes are
//! captured in the JSON report. The caller decides whether to exit non-zero
//! based on the returned `bool` flag. This module is also called by `init`
//! as a final verification step after onboarding.

use serde_json::{json, Value};

use crate::auth::{self, OAuthConfig};
use crate::client::ConfluenceClient;
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

    let oauth_scopes_check = match credentials {
        Some(ref creds) if creds_passed => check_oauth_scopes(creds),
        _ => skipped("credentials check failed"),
    };
    let oauth_scopes_passed = oauth_scopes_check["status"] == "ok";

    let all_ok = app_passed && creds_passed && connectivity_passed && oauth_scopes_passed;

    let report = json!({
        "app_config": app_check,
        "credentials": creds_check,
        "api": connectivity_check,
        "oauth_scopes": oauth_scopes_check,
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
                "message": format!("credentials file not found at {path_str}. Run: confluence auth login")
            }),
            None,
        );
    };

    let Ok(credentials) = serde_json::from_str::<auth::Credentials>(&raw) else {
        return (
            json!({
                "status": "error",
                "path": path_str,
                "message": "credentials file is malformed. Run: confluence auth login"
            }),
            None,
        );
    };

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    if now >= credentials.expires_at {
        return match auth::renew(oauth_config, &credentials) {
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
                    "message": format!("token expired and renewal failed: {e}. Run: confluence auth login")
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
    let client = ConfluenceClient::new(credentials);
    match client.get_current_user() {
        Ok(user) => {
            let display_name = user["displayName"].as_str().unwrap_or("unknown").to_string();
            let account_id = user["accountId"].as_str().unwrap_or("unknown").to_string();
            json!({"status": "ok", "display_name": display_name, "account_id": account_id})
        }
        Err(e) => json!({"status": "error", "message": e.to_string()}),
    }
}

/// Lists the OAuth scopes granted to the token (the app-identity layer), via
/// the accessible-resources endpoint. `status` is `"error"` if the list is
/// empty — an empty scope list means no Confluence API call can succeed.
/// Otherwise purely informational.
fn check_oauth_scopes(credentials: &auth::Credentials) -> Value {
    match auth::get_granted_scopes(&credentials.access_token, &credentials.cloud_id) {
        Ok(scopes) => {
            let status = if scopes.is_empty() { "error" } else { "ok" };
            json!({"status": status, "granted": scopes})
        }
        Err(e) => json!({"status": "error", "message": e.to_string()}),
    }
}

fn skipped(reason: &str) -> Value {
    json!({"status": "skipped", "reason": reason})
}
