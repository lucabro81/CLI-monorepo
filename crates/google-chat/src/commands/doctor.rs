//! Handler for the `doctor` command.
//!
//! Runs sequential checks and returns a structured JSON report:
//!
//! 1. `app_config` — verifies that `app.json` exists at the expected path and
//!    contains valid OAuth credentials.
//! 2. `credentials` — verifies that `credentials.json` exists and holds a
//!    non-expired token. If the token is expired, a renewal is attempted and
//!    the result (success or failure) is reported transparently.
//! 3. `api` — makes a live call to `spaces.list` (`pageSize=1`) to confirm
//!    the Chat API is reachable with the current token.
//!
//! Unlike jira, there is no separate `oauth_scopes`/`service_user`/`projects`
//! layer: Google Chat authorizes purely by OAuth scope, with no Jira-style
//! per-site permission system to probe independently of the token itself.
//!
//! Checks cascade: if `app_config` fails, `credentials` and `api` are marked
//! `skipped` (no credentials to load). If `credentials` fails, `api` is
//! skipped (no token to use).
//!
//! The function never returns `Err` for check failures — all outcomes are
//! captured in the JSON report. The caller decides whether to exit non-zero
//! based on the returned `bool` flag. This module is also called by `init`
//! as a final verification step after onboarding.

use serde_json::{json, Value};

use crate::auth::{self, OAuthConfig};
use crate::client::GoogleChatClient;
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

    let live_check = match credentials {
        Some(ref creds) if creds_passed => check_api(creds),
        _ => skipped("credentials check failed"),
    };
    let live_passed = live_check["status"] == "ok";

    let all_ok = app_passed && creds_passed && live_passed;

    let report = json!({
        "app_config": app_check,
        "credentials": creds_check,
        "api": live_check,
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
                "message": format!("credentials file not found at {path_str}. Run: google-chat auth login")
            }),
            None,
        );
    };

    let Ok(credentials) = serde_json::from_str::<auth::Credentials>(&raw) else {
        return (
            json!({
                "status": "error",
                "path": path_str,
                "message": "credentials file is malformed. Run: google-chat auth login"
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
                    "message": format!("token expired and renewal failed: {e}. Run: google-chat auth login")
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
    let client = GoogleChatClient::new(credentials);
    match client.list_spaces(1, None) {
        Ok(_) => json!({"status": "ok"}),
        Err(e) => json!({"status": "error", "message": e.to_string()}),
    }
}

fn skipped(reason: &str) -> Value {
    json!({"status": "skipped", "reason": reason})
}
