//! Handler for the `doctor` command.
//!
//! Runs sequential checks and returns a structured JSON report:
//!
//! 1. `app_config` — verifies that `app.json` exists at the expected path and
//!    contains valid OAuth credentials.
//! 2. `credentials` — verifies that `credentials.json` exists and holds a
//!    non-expired token. If the token is expired, a renewal is attempted and
//!    the result (success or failure) is reported transparently.
//! 3. `api` — makes a live call to `/rest/api/3/myself` to confirm the Jira
//!    API is reachable with the current token.
//! 4. `oauth_scopes` — lists the OAuth scopes granted to the token (the app
//!    identity layer), via the accessible-resources endpoint.
//! 5. `service_user` — lists the global Jira permissions granted to the
//!    authenticated account, via `/rest/api/3/mypermissions` (no project
//!    context).
//! 6. `projects` — for every project visible to the account, lists which
//!    project roles the account belongs to and which Jira permissions it
//!    holds in that project (`/rest/api/3/mypermissions?projectKey=...`).
//!    A project with no roles/permissions is reported with `status: "error"`;
//!    finding zero projects at all is also `status: "error"` (an account
//!    that can't see any project can't do anything useful).
//!
//! Checks cascade: if `app_config` fails, the remaining checks are marked
//! `skipped` (no credentials to load). If `credentials` fails, `api`,
//! `oauth_scopes`, `service_user` and `projects` are skipped (no token to
//! use). The other checks do not depend on `api` and run whenever
//! `credentials` succeeds.
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
    let account_id = jira_check["account_id"].as_str().map(str::to_string);

    let oauth_scopes_check = match credentials {
        Some(ref creds) if creds_passed => check_oauth_scopes(creds),
        _ => skipped("credentials check failed"),
    };
    let oauth_scopes_passed = oauth_scopes_check["status"] == "ok";

    let service_user_check = match credentials {
        Some(ref creds) if creds_passed => check_service_user(creds),
        _ => skipped("credentials check failed"),
    };
    let service_user_passed = service_user_check["status"] == "ok";

    let projects_check = match (&credentials, &account_id) {
        (Some(creds), Some(account_id)) if creds_passed => check_projects(creds, account_id),
        _ if creds_passed => skipped("could not resolve account id (api check failed)"),
        _ => skipped("credentials check failed"),
    };
    let projects_passed = projects_check["status"] == "ok";

    let all_ok = app_passed
        && creds_passed
        && jira_passed
        && oauth_scopes_passed
        && service_user_passed
        && projects_passed;

    let report = json!({
        "app_config": app_check,
        "credentials": creds_check,
        "api": jira_check,
        "oauth_scopes": oauth_scopes_check,
        "service_user": service_user_check,
        "projects": projects_check,
    });

    Ok((report, all_ok))
}

/// Jira permission keys checked by the `service_user` and `projects` doctor
/// checks. These are the permissions the CLI's `issue` commands rely on.
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
            let account_id = user["accountId"].as_str().unwrap_or("unknown").to_string();
            json!({"status": "ok", "account": account, "email": email, "account_id": account_id})
        }
        Err(e) => json!({"status": "error", "message": e.to_string()}),
    }
}

/// Lists the OAuth scopes granted to the token (the app-identity layer), via
/// the accessible-resources endpoint. `status` is `"error"` if the list is
/// empty — an empty scope list means no Jira API call can succeed regardless
/// of any Jira-side permission. Otherwise purely informational.
fn check_oauth_scopes(credentials: &auth::Credentials) -> Value {
    match auth::get_granted_scopes(&credentials.access_token, &credentials.cloud_id) {
        Ok(scopes) => {
            let status = if scopes.is_empty() { "error" } else { "ok" };
            json!({"status": status, "granted": scopes})
        }
        Err(e) => json!({"status": "error", "message": e.to_string()}),
    }
}

/// Lists which of `PERMISSION_KEYS` are granted to the authenticated account
/// globally (no project context), via `/rest/api/3/mypermissions`. `status`
/// is `"error"` if none are granted — without any global permission, no
/// `issue` command can do anything useful in any project.
fn check_service_user(credentials: &auth::Credentials) -> Value {
    let client = JiraClient::new(credentials);
    match client.get_my_permissions(PERMISSION_KEYS, None) {
        Ok(response) => {
            let granted = granted_permission_keys(&response);
            let status = if granted.is_empty() { "error" } else { "ok" };
            json!({"status": status, "global_permissions": granted})
        }
        Err(e) => json!({"status": "error", "message": e.to_string()}),
    }
}

/// For every project visible to the account, reports which project roles the
/// account belongs to and which of `PERMISSION_KEYS` it holds in that
/// project's permission scheme. `status` is `"error"` if zero projects are
/// visible at all (an account that can't see any project can't do anything
/// useful), or if any individual project has no roles/permissions.
fn check_projects(credentials: &auth::Credentials, account_id: &str) -> Value {
    let client = JiraClient::new(credentials);

    let project_keys = match client.list_projects() {
        Ok(keys) => keys,
        Err(e) => return json!({"status": "error", "message": e.to_string()}),
    };

    if project_keys.is_empty() {
        return json!({
            "status": "error",
            "message": "no projects visible to this account. Without at least one visible project, no issue command can do anything. Check the account's project access in Jira."
        });
    }

    let mut report = serde_json::Map::new();
    let mut all_ok = true;

    for key in &project_keys {
        let project_report = check_project(&client, key, account_id);
        if project_report["status"] != "ok" {
            all_ok = false;
        }
        report.insert(key.clone(), project_report);
    }

    report.insert(
        "status".to_string(),
        Value::String(if all_ok { "ok" } else { "error" }.to_string()),
    );
    Value::Object(report)
}

/// Checks a single project: which `PERMISSION_KEYS` `account_id` holds in
/// that project ([`JiraClient::get_my_permissions`] with `projectKey` set,
/// can differ from `service_user`'s global permissions), and which project
/// roles it belongs to ([`JiraClient::get_project_roles`] +
/// [`JiraClient::get_role_actor_account_ids`]).
///
/// Listing project roles requires the "Administer Projects" permission,
/// which the account may not have for every project it can otherwise use.
/// In that case `service_user_roles` is `null` with an explanatory note,
/// rather than failing the whole project — `status` is based only on
/// `service_user_permissions`.
fn check_project(client: &JiraClient, project_key: &str, account_id: &str) -> Value {
    let permissions = match client.get_my_permissions(PERMISSION_KEYS, Some(project_key)) {
        Ok(response) => granted_permission_keys(&response),
        Err(e) => return json!({"status": "error", "message": format!("failed to fetch permissions: {e}")}),
    };

    let status = if permissions.is_empty() { "error" } else { "ok" };

    let (roles, roles_note) = match client.get_project_roles(project_key) {
        Ok(roles) => {
            let mut member_roles = Vec::new();
            for (role_name, role_url) in &roles {
                match client.get_role_actor_account_ids(role_url) {
                    Ok(account_ids) if account_ids.iter().any(|id| id == account_id) => {
                        member_roles.push(role_name.clone());
                    }
                    Ok(_) => {}
                    Err(e) => {
                        return json!({
                            "status": "error",
                            "message": format!("failed to fetch actors for role {role_name}: {e}")
                        })
                    }
                }
            }
            (Some(member_roles), None)
        }
        Err(e) => (
            None,
            Some(format!(
                "could not list project roles (requires Administer Projects permission): {e}"
            )),
        ),
    };

    json!({
        "status": status,
        "service_user_permissions": permissions,
        "service_user_roles": roles,
        "service_user_roles_note": roles_note,
    })
}

/// Returns the subset of `PERMISSION_KEYS` that have `havePermission: true` in
/// a `/rest/api/3/mypermissions` response.
fn granted_permission_keys(response: &Value) -> Vec<String> {
    PERMISSION_KEYS
        .iter()
        .filter(|key| {
            response["permissions"][key]["havePermission"]
                .as_bool()
                .unwrap_or(false)
        })
        .map(|key| (*key).to_string())
        .collect()
}

fn skipped(reason: &str) -> Value {
    json!({"status": "skipped", "reason": reason})
}
