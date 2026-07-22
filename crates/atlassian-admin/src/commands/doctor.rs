//! Handler for the `doctor` command.
//!
//! Runs two sequential checks and returns a structured JSON report:
//!
//! 1. `app_config` — verifies that `app.json` exists at the expected path and
//!    contains a well-formed `api_key`/`org_id` pair.
//! 2. `api` — makes a live call to `GET /v1/orgs/{org_id}` to confirm the key
//!    and org id work together against the real API. Chosen over a full user
//!    lookup because it needs no `account_id` input and is cheap.
//!
//! Checks cascade: if `app_config` fails, `api` is marked `skipped` (nothing
//! to call with). Unlike jira/bitbucket, there is no `credentials`/`permissions`
//! check — this crate has no token exchange and no introspectable scope list
//! for a static API key (see crate CLAUDE.md's "Auth design").
//!
//! The function never returns `Err` for check failures — all outcomes are
//! captured in the JSON report. The caller decides whether to exit non-zero
//! based on the returned `bool` flag.

use serde_json::{json, Value};

use crate::auth::AdminConfig;
use crate::client::AdminClient;
use crate::context::config_dir;
use crate::error::CliError;

/// Runs all doctor checks. Returns `(report, all_ok)`.
///
/// `report` is a JSON object with one key per check. `all_ok` is `true` only
/// if every check has `status: "ok"`.
pub fn run_doctor() -> Result<(Value, bool), CliError> {
    let config_dir = config_dir()?;

    let (app_check, admin_config) = check_app_config(&config_dir);
    let app_passed = app_check["status"] == "ok";

    let org_check = match admin_config {
        Some(ref config) if app_passed => check_api(config),
        _ => skipped("app_config check failed"),
    };
    let org_passed = org_check["status"] == "ok";

    let all_ok = app_passed && org_passed;

    let report = json!({
        "app_config": app_check,
        "api": org_check,
    });

    Ok((report, all_ok))
}

fn check_app_config(config_dir: &std::path::Path) -> (Value, Option<AdminConfig>) {
    let path = crate::auth::app_config_path(config_dir);
    let path_str = path.display().to_string();

    match AdminConfig::load(&path) {
        Ok(config) => (json!({"status": "ok", "path": path_str}), Some(config)),
        Err(e) => (
            json!({"status": "error", "path": path_str, "message": e.to_string()}),
            None,
        ),
    }
}

fn check_api(config: &AdminConfig) -> Value {
    let client = AdminClient::new(config);
    match client.get_organization() {
        Ok(org) => {
            let name = org["data"]["attributes"]["name"].as_str().unwrap_or("unknown").to_string();
            json!({"status": "ok", "organization_name": name})
        }
        Err(e) => json!({"status": "error", "message": e.to_string()}),
    }
}

fn skipped(reason: &str) -> Value {
    json!({"status": "skipped", "reason": reason})
}
