//! Handler for the `doctor` command.
//!
//! Runs four sequential checks and returns a structured JSON report:
//!
//! 1. `app_config` — verifies that `app.json` exists at the expected path and
//!    contains valid OAuth credentials.
//! 2. `credentials` — verifies that `credentials.json` exists and holds a
//!    non-expired token. If the token is expired, a renewal is attempted and
//!    the result (success or failure) is reported transparently.
//! 3. `api` — makes a live call to `/rest/api/3/myself` to confirm the Jira
//!    API is reachable with the current token.
//! 4. `permissions` — calls `/rest/api/3/mypermissions` to report which Jira
//!    permissions (as opposed to OAuth scopes) are actually granted.
//!
//! Checks cascade: if `app_config` fails, the remaining checks are marked
//! `skipped` (no credentials to load). If `credentials` fails, `api` and
//! `permissions` are skipped (no token to use). `permissions` does not
//! depend on `api` and runs whenever `credentials` succeeds.
//!
//! The function never returns `Err` for check failures — all outcomes are
//! captured in the JSON report. The caller decides whether to exit non-zero
//! based on the returned `bool` flag. This module is also called by `init`
//! as a final verification step after onboarding.

use serde_json::{json, Value};

use crate::auth::{self, OAuthConfig};
use crate::client::JiraClient;
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

    let jira_check = match credentials {
        Some(ref creds) if creds_passed => check_api(creds),
        _ => skipped("credentials check failed"),
    };
    let jira_passed = jira_check["status"] == "ok";

    let permissions_check = match credentials {
        Some(ref creds) if creds_passed => check_permissions(creds),
        _ => skipped("credentials check failed"),
    };
    let permissions_passed = permissions_check["status"] == "ok";

    let all_ok = app_passed && creds_passed && jira_passed && permissions_passed;

    let report = json!({
        "app_config": app_check,
        "credentials": creds_check,
        "api": jira_check,
        "permissions": permissions_check,
    });

    Ok((report, all_ok))
}

/// Jira permission keys checked by the `permissions` doctor check. These are
/// the project-level permissions the CLI's `issue` commands rely on.
const PERMISSION_KEYS: &[&str] = &[
    "BROWSE_PROJECTS",
    "CREATE_ISSUES",
    "EDIT_ISSUES",
    "DELETE_ISSUES",
    "ADD_COMMENTS",
    "TRANSITION_ISSUES",
];

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
                "message": "credentials file is malformed. Run: jira auth login"
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
                    "message": format!("token expired and renewal failed: {e}. Run: jira auth login")
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

/// Reports which Jira permissions (distinct from OAuth scopes) are actually
/// granted to the authenticated account, via `/rest/api/3/mypermissions`.
///
/// `status` is `"ok"` only if `BROWSE_PROJECTS` is granted — without it, no
/// `issue` command can do anything useful. The other permission keys are
/// reported informationally regardless of their value, so an LLM can see
/// exactly which `issue` subcommands are available.
fn check_permissions(credentials: &auth::Credentials) -> Value {
    let client = JiraClient::new(credentials);
    match client.get_my_permissions(PERMISSION_KEYS) {
        Ok(response) => {
            let permissions = PERMISSION_KEYS
                .iter()
                .map(|key| {
                    let granted = response["permissions"][key]["havePermission"]
                        .as_bool()
                        .unwrap_or(false);
                    ((*key).to_string(), Value::Bool(granted))
                })
                .collect::<serde_json::Map<_, _>>();

            let browse_projects = permissions["BROWSE_PROJECTS"].as_bool().unwrap_or(false);
            let status = if browse_projects { "ok" } else { "error" };

            json!({"status": status, "permissions": permissions})
        }
        Err(e) => json!({"status": "error", "message": e.to_string()}),
    }
}

fn skipped(reason: &str) -> Value {
    json!({"status": "skipped", "reason": reason})
}
