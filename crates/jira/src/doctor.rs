use std::path::Path;

use serde_json::{json, Value};

use crate::auth::{self, OAuthConfig};
use crate::client::JiraClient;
use crate::context::config_dir;
use crate::error::CliError;

/// Runs all doctor checks and returns a structured JSON report plus an overall pass/fail flag.
/// Never fails with `CliError` for check failures — all failures are captured in the JSON.
pub fn run_doctor() -> Result<(Value, bool), CliError> {
    let config_dir = config_dir()?;

    let (app_check, oauth_config) = check_app_config(&config_dir);
    let app_passed = app_check["status"] == "ok";

    let (creds_check, credentials) = match oauth_config {
        Some(ref config) if app_passed => check_credentials(config, &config_dir),
        _ => (skipped("app_config check failed"), None),
    };
    let creds_passed = creds_check["status"] == "ok";

    let jira_check = match credentials {
        Some(ref creds) if creds_passed => check_api(creds),
        _ => skipped("credentials check failed"),
    };
    let jira_passed = jira_check["status"] == "ok";

    let all_ok = app_passed && creds_passed && jira_passed;

    let report = json!({
        "app_config": app_check,
        "credentials": creds_check,
        "api": jira_check,
    });

    Ok((report, all_ok))
}

fn check_app_config(config_dir: &Path) -> (Value, Option<OAuthConfig>) {
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
    config_dir: &Path,
) -> (Value, Option<auth::Credentials>) {
    let path = auth::credentials_path(config_dir);
    let path_str = path.display().to_string();

    let Ok(raw) = std::fs::read_to_string(&path) else {
        return (
            json!({
                "status": "error",
                "path": path_str,
                "message": format!("credentials file not found at {path_str}. Run: jira auth login")
            }),
            None,
        );
    };

    let Ok(credentials) = serde_json::from_str::<auth::Credentials>(&raw) else {
        return (
            json!({
                "status": "error",
                "path": path_str,
                "message": format!("credentials file is malformed. Run: jira auth login")
            }),
            None,
        );
    };

    // Check expiry without silently refreshing — if expired, try once and report.
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    if now >= credentials.expires_at {
        return match auth::refresh(oauth_config, &credentials) {
            Ok(refreshed) => {
                let _ = auth::save_credentials(&path, &refreshed);
                (
                    json!({
                        "status": "ok",
                        "path": path_str,
                        "expires_at": refreshed.expires_at,
                        "note": "token was expired and has been refreshed"
                    }),
                    Some(refreshed),
                )
            }
            Err(e) => (
                json!({
                    "status": "error",
                    "path": path_str,
                    "message": format!("token expired and refresh failed: {e}. Run: jira auth login")
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
    let client = JiraClient::new(credentials);
    match client.get_myself() {
        Ok(user) => {
            let account = user["displayName"].as_str().unwrap_or("unknown").to_string();
            let email = user["emailAddress"].as_str().unwrap_or("unknown").to_string();
            json!({"status": "ok", "account": account, "email": email})
        }
        Err(e) => json!({"status": "error", "message": e.to_string()}),
    }
}

fn skipped(reason: &str) -> Value {
    json!({"status": "skipped", "reason": reason})
}
