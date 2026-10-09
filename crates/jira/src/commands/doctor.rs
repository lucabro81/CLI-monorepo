//! Handler for the `doctor` command.
//!
//! Runs sequential checks and returns a structured JSON report:
//!
//! Checks 1-6 run for one identity: the Service Account, or the person with
//! `--user <id>`.
//!
//! 1. `app_config` — verifies that `app.json` exists at the expected path and
//!    has a valid section for the identity.
//! 2. `credentials` — verifies that the identity's credentials file
//!    (`credentials-service.json` / `users/<id>/credentials.json`) exists and holds a
//!    non-expired token. If the token is expired, a renewal is attempted (under
//!    the same lock as every command's) and the result is reported transparently.
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
//! 7. `pending_login` — informational, outside the cascade and `all_ok`:
//!    whether the selected person's two-step `auth login --user <id> --remote`
//!    is waiting for its code (always `none` for the Service Account).
//! 8. `identities` — informational, outside the cascade and `all_ok`, no
//!    network: the identity checked, whether the Service Account is logged in,
//!    the ids of the people logged in, and credentials files of earlier layouts
//!    still lying around.
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

use crate::auth::{self, AppConfig, Identity, OAuthConfig};
use crate::client::{ClientError, JiraClient};
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

    let (mut creds_check, mut credentials) = match oauth_config {
        Some(ref config) if app_passed => check_credentials(config, &config_dir, identity),
        _ => (skipped("app_config check failed"), None),
    };
    let mut creds_passed = creds_check["status"] == "ok";

    let jira_check = match (&oauth_config, credentials.as_mut()) {
        (Some(config), Some(creds)) if creds_passed => {
            let path = auth::credentials_path(&config_dir, identity);
            let client = JiraClient::new(creds)
                .with_renewer(rejected_token_renewer(config.clone(), path.clone(), identity.clone()));
            match api_or_credentials_error(&client, &path) {
                Ok(check) => {
                    // Renewed after a 401: the later checks need the new token.
                    creds.access_token = client.access_token();
                    check
                }
                Err(check) => {
                    creds_check = check;
                    creds_passed = false;
                    skipped("credentials check failed")
                }
            }
        }
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
        "pending_login": check_pending_login(&config_dir, identity, atlassian_auth::now_unix()),
        "identities": check_identities(&config_dir, identity),
    });

    Ok((report, all_ok))
}

/// Jira permission keys checked by the `service_user` and `projects` doctor
/// checks. These are the permissions the CLI's `issue` and `user` commands rely
/// on. `USER_PICKER` ("Browse users and groups") is a global-only permission —
/// unlike the others it isn't part of any project's permission scheme, so it
/// reads the same in every project's report — but it's included here rather
/// than in a separate check because `jira user search` fails silently (an empty
/// match list, not an error) when it's missing, making it worth surfacing.
const PERMISSION_KEYS: &[&str] = &[
    "BROWSE_PROJECTS",
    "CREATE_ISSUES",
    "EDIT_ISSUES",
    "DELETE_ISSUES",
    "ADD_COMMENTS",
    "TRANSITION_ISSUES",
    "USER_PICKER",
    "ASSIGN_ISSUES",
];

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
    identity: &Identity,
) -> (Value, Option<auth::Credentials>) {
    let path = auth::credentials_path(config_dir, identity);
    let path_str = path.display().to_string();
    let login = login_command(identity);

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

    if let Err(e) = atlassian_auth::check_identity(&credentials, identity) {
        return (
            json!({"status": "error", "path": path_str, "message": format!("{e}. Run: {login}")}),
            None,
        );
    }

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    if now >= credentials.expires_at {
        // load_credentials renews under the per-file lock, like every command.
        return match auth::load_credentials(oauth_config, &path, identity) {
            Ok(renewed) => (
                json!({
                    "status": "ok",
                    "path": path_str,
                    "expires_at": renewed.expires_at,
                    "note": "token was expired and has been renewed"
                }),
                Some(renewed),
            ),
            Err(e) => (
                json!({
                    "status": "error",
                    "path": path_str,
                    "message": format!("token expired and renewal failed: {e}. Run: {login}")
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

/// The `api` check; `Err` is the `credentials` check to report instead when
/// the API answered 401 and the token could not be renewed (issue #240: a
/// token revoked before it expired is a credentials problem, not an API one).
pub(crate) fn api_or_credentials_error(client: &JiraClient, credentials_path: &std::path::Path) -> Result<Value, Value> {
    check_api(client).map_err(|renewal| {
        json!({
            "status": "error",
            "path": credentials_path.display().to_string(),
            "message": renewal.to_string(),
        })
    })
}

/// The `api` check, or the error of a renewal after a 401.
fn check_api(client: &JiraClient) -> Result<Value, CliError> {
    Ok(match client.get_myself() {
        Ok(user) => {
            let account = user["displayName"].as_str().unwrap_or("unknown").to_string();
            let email = user["emailAddress"].as_str().unwrap_or("unknown").to_string();
            let account_id = user["accountId"].as_str().unwrap_or("unknown").to_string();
            json!({"status": "ok", "account": account, "email": email, "account_id": account_id})
        }
        Err(ClientError::Renewal(e)) => return Err(*e),
        Err(e) => json!({"status": "error", "message": e.to_string()}),
    })
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

/// Informational: the selected person's two-step (`--remote`) login waiting
/// for its code. Never affects `all_ok` (an idle CLI has none, and that is
/// fine); the Service Account never has one.
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
            "message": format!(
                "{e}. Start a new remote login with: jira auth login --user {id} --remote --redirect-uri <redirect-uri>"
            ),
        }),
    }
}

/// Informational, no network: which identity this report checks, whether
/// the Service Account is logged in, the ids of the people logged in, and
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
