//! End-to-end tests against a real Jira Cloud instance.
//!
//! # Prerequisites
//!
//! - `jira auth login` must have been run on this machine.
//! - The env var `JIRA_E2E_PROJECT` must be set to a writable Jira project key (e.g. `KAN`).
//!
//! # Running
//!
//! ```sh
//! JIRA_E2E_PROJECT=KAN cargo test -p jira -- --ignored
//! ```
//!
//! Run a single test:
//! ```sh
//! JIRA_E2E_PROJECT=KAN cargo test -p jira e2e_cleanup -- --ignored
//! ```
//!
//! # Isolation
//!
//! Every issue created by these tests has `[jira-cli-e2e]` as a summary prefix.
//! `IssueGuard` deletes the issue on drop (even on test failure or panic).
//! If a guard is skipped due to a panic before the guard is created, run
//! `e2e_cleanup` to delete all orphaned test issues.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use serde_json::Value;

use crate::auth::{self, Credentials};
use crate::client::JiraClient;
use crate::context;
use crate::fields::filter_fields;

// ── Constants ──────────────────────────────────────────────────────────────

const E2E_PREFIX: &str = "[jira-cli-e2e]";

// ── Helpers ─────────────────────────────────────────────────────────────────

/// Returns the project key from the environment, panicking with a clear message if unset.
fn project_key() -> String {
    std::env::var("JIRA_E2E_PROJECT")
        .expect("set JIRA_E2E_PROJECT to a writable Jira project key before running e2e tests")
}

/// Builds an authenticated `JiraClient` and returns the credentials alongside it
/// so they can be stored in an `IssueGuard`.
fn setup() -> (JiraClient, Credentials) {
    let config_dir = context::config_dir().expect("could not resolve config dir");
    let oauth_config = auth::OAuthConfig::load(&auth::app_config_path(&config_dir))
        .expect("app.json not found — run `jira init` first");
    let credentials =
        auth::load_credentials(&oauth_config, &auth::credentials_path(&config_dir))
            .expect("not authenticated — run `jira auth login` first");
    let client = JiraClient::new(&credentials);
    (client, credentials)
}

/// Builds an e2e issue summary with a timestamp suffix for uniqueness.
fn e2e_summary(label: &str) -> String {
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    format!("{E2E_PREFIX} {label} {ts}")
}

/// Applies dot-notation `--select`-style projection to a value. Convenience wrapper
/// over `filter_fields` for use in assertions.
fn select(value: Value, paths: &str) -> Value {
    let parts: Vec<&str> = paths.split(',').collect();
    filter_fields(value, &parts)
}

// ── IssueGuard ──────────────────────────────────────────────────────────────

/// RAII guard that deletes a Jira issue on drop.
///
/// Deletion is best-effort: errors (including 404 if the issue was already
/// deleted by the test itself) are silently ignored. Always passes
/// `delete_subtasks=true` to avoid 400 errors on issues with subtasks.
struct IssueGuard {
    key: String,
    credentials: Credentials,
}

impl IssueGuard {
    fn new(key: impl Into<String>, credentials: Credentials) -> Self {
        Self { key: key.into(), credentials }
    }
}

impl Drop for IssueGuard {
    fn drop(&mut self) {
        let client = JiraClient::new(&self.credentials);
        // 404 means already deleted — that's fine. Any other error is also
        // silently swallowed since we cannot propagate errors from Drop.
        let _ = client.delete_issue(&self.key, true);
    }
}

// ── Smoke tests ─────────────────────────────────────────────────────────────

#[test]
#[ignore = "e2e: requires credentials and JIRA_E2E_PROJECT"]
fn e2e_smoke_doctor() {
    let (report, all_ok) = crate::commands::doctor::run_doctor()
        .expect("doctor should not fail with a CliError");

    assert!(all_ok, "doctor reported a failing check: {report}");
    assert_eq!(report["app_config"]["status"], "ok");
    assert_eq!(report["credentials"]["status"], "ok");
    assert_eq!(report["api"]["status"], "ok");
}

#[test]
#[ignore = "e2e: requires credentials and JIRA_E2E_PROJECT"]
fn e2e_smoke_whoami() {
    let (client, _) = setup();
    let user = client.get_myself().expect("get_myself should succeed");

    assert!(user["accountId"].is_string(), "response must contain accountId");
    assert!(user["displayName"].is_string(), "response must contain displayName");
}

// ── Issue lifecycle ──────────────────────────────────────────────────────────

#[test]
#[ignore = "e2e: requires credentials and JIRA_E2E_PROJECT"]
fn e2e_issue_lifecycle() {
    let (client, creds) = setup();
    let project = project_key();
    let summary = e2e_summary("lifecycle");

    // Create
    let created = client
        .create_issue(&project, "Task", &summary, None, None, None)
        .expect("create_issue should succeed");
    let key = created["key"].as_str().expect("response must contain key").to_string();
    let _guard = IssueGuard::new(&key, creds);

    // Get
    let issue = client.get_issue(&key).expect("get_issue should succeed");
    assert_eq!(issue["fields"]["summary"], summary);
    assert_eq!(issue["key"], key);

    // Status must be non-empty
    assert!(
        issue["fields"]["status"]["name"].is_string(),
        "issue must have a status"
    );
}

#[test]
#[ignore = "e2e: requires credentials and JIRA_E2E_PROJECT"]
fn e2e_issue_select_projection() {
    let (client, creds) = setup();
    let project = project_key();
    let summary = e2e_summary("select-projection");

    let created = client
        .create_issue(&project, "Task", &summary, None, None, None)
        .expect("create_issue should succeed");
    let key = created["key"].as_str().expect("key missing").to_string();
    let _guard = IssueGuard::new(&key, creds);

    let issue = client.get_issue(&key).expect("get_issue should succeed");

    // Apply --select equivalent: fields.summary,fields.status.name
    let projected = select(issue, "key,fields.summary,fields.status.name");

    // Requested fields present
    assert_eq!(projected["key"], key);
    assert_eq!(projected["fields"]["summary"], summary);
    assert!(projected["fields"]["status"]["name"].is_string());

    // Unrequested fields absent
    assert!(projected["fields"]["assignee"].is_null() || !projected["fields"].get("assignee").is_some(),
        "assignee should not appear in projected output");
    assert!(projected["fields"].get("description").is_none()
        || projected["fields"]["description"].is_null());
}

// ── Comment lifecycle ────────────────────────────────────────────────────────

#[test]
#[ignore = "e2e: requires credentials and JIRA_E2E_PROJECT"]
fn e2e_comment_lifecycle() {
    let (client, creds) = setup();
    let project = project_key();
    let summary = e2e_summary("comment-lifecycle");
    let comment_body = format!("{E2E_PREFIX} test comment body");

    // Create issue
    let created = client
        .create_issue(&project, "Task", &summary, None, None, None)
        .expect("create_issue should succeed");
    let key = created["key"].as_str().expect("key missing").to_string();
    let _guard = IssueGuard::new(&key, creds);

    // Add comment
    let comment = client
        .add_comment(&key, &comment_body)
        .expect("add_comment should succeed");
    let comment_id = comment["id"].as_str().expect("comment id missing").to_string();

    // Verify comment present via issue get --select
    let issue = client.get_issue(&key).expect("get_issue should succeed");
    let comments = &issue["fields"]["comment"]["comments"];
    let found = comments
        .as_array()
        .expect("comments must be an array")
        .iter()
        .any(|c| c["id"] == comment_id);
    assert!(found, "comment {comment_id} not found in issue comments");

    // Remove comment
    client
        .delete_comment(&key, &comment_id)
        .expect("delete_comment should succeed");

    // Verify comment gone
    let issue = client.get_issue(&key).expect("get_issue after delete should succeed");
    let comments_after = issue["fields"]["comment"]["comments"]
        .as_array()
        .expect("comments must be an array");
    let still_present = comments_after.iter().any(|c| c["id"] == comment_id);
    assert!(!still_present, "comment {comment_id} should have been deleted");
}

// ── Transition ───────────────────────────────────────────────────────────────

#[test]
#[ignore = "e2e: requires credentials and JIRA_E2E_PROJECT"]
fn e2e_transition() {
    let (client, creds) = setup();
    let project = project_key();
    let summary = e2e_summary("transition");

    // Create issue
    let created = client
        .create_issue(&project, "Task", &summary, None, None, None)
        .expect("create_issue should succeed");
    let key = created["key"].as_str().expect("key missing").to_string();
    let _guard = IssueGuard::new(&key, creds);

    // Get initial status
    let issue = client.get_issue(&key).expect("get_issue should succeed");
    let initial_status = issue["fields"]["status"]["name"]
        .as_str()
        .expect("status name missing")
        .to_string();

    // Get available transitions
    let transitions = client
        .get_transitions(&key)
        .expect("get_transitions should succeed");
    assert!(!transitions.is_empty(), "issue must have at least one transition available");

    // Pick first transition that leads to a different status than the current one
    let target = transitions
        .iter()
        .find(|t| !t.name.eq_ignore_ascii_case(&initial_status))
        .unwrap_or(&transitions[0]);

    // Apply transition
    client
        .apply_transition(&key, &target.id)
        .expect("apply_transition should succeed");

    // Verify new status
    let issue_after = client.get_issue(&key).expect("get_issue after transition should succeed");
    let new_status = issue_after["fields"]["status"]["name"]
        .as_str()
        .expect("status name missing after transition");
    assert_eq!(
        new_status, target.name,
        "issue status should match the applied transition"
    );
}

// ── Search ───────────────────────────────────────────────────────────────────

#[test]
#[ignore = "e2e: requires credentials and JIRA_E2E_PROJECT"]
fn e2e_search_simple() {
    let (client, creds) = setup();
    let project = project_key();
    let summary = e2e_summary("search-simple");

    let created = client
        .create_issue(&project, "Task", &summary, None, None, None)
        .expect("create_issue should succeed");
    let key = created["key"].as_str().expect("key missing").to_string();
    let _guard = IssueGuard::new(&key, creds);

    // Search by exact key
    let jql = format!("issue = {key}");
    let results = client
        .search_issues(&jql, 10, None, Some("summary,status"))
        .expect("search should succeed");

    let issues = results["issues"].as_array().expect("issues must be array");
    assert_eq!(issues.len(), 1, "should find exactly one issue");
    assert_eq!(issues[0]["key"], key);
    assert_eq!(issues[0]["fields"]["summary"], summary);
}

#[test]
#[ignore = "e2e: requires credentials and JIRA_E2E_PROJECT"]
fn e2e_search_complex() {
    let (client, creds) = setup();
    let project = project_key();
    let summary = e2e_summary("search-complex");

    let created = client
        .create_issue(&project, "Task", &summary, None, None, None)
        .expect("create_issue should succeed");
    let key = created["key"].as_str().expect("key missing").to_string();
    let _guard = IssueGuard::new(&key, creds);

    // Multi-condition JQL: exact key + type + status + ORDER BY.
    // Deliberately avoids `summary ~` (text search) which requires Jira indexing
    // and would make the test flaky on freshly created issues.
    let issue = client.get_issue(&key).expect("get_issue should succeed");
    let status_name = issue["fields"]["status"]["name"]
        .as_str()
        .expect("status name missing")
        .to_string();

    let jql = format!(
        "issue = {key} AND issuetype = Task AND status = \"{status_name}\" ORDER BY created DESC"
    );
    let results = client
        .search_issues(&jql, 50, None, Some("summary,status,issuetype"))
        .expect("search should succeed");

    let issues = results["issues"].as_array().expect("issues must be array");
    assert_eq!(issues.len(), 1, "complex search should return exactly our issue");
    assert_eq!(issues[0]["key"], key);
    assert_eq!(issues[0]["fields"]["issuetype"]["name"], "Task");
}

#[test]
#[ignore = "e2e: requires credentials and JIRA_E2E_PROJECT"]
fn e2e_search_pagination() {
    // Self-contained: creates two issues of its own and paginates over exactly
    // those two via `issue in (...)`, so the result set is unaffected by issues
    // created/deleted by other tests running concurrently.
    let (client, creds) = setup();
    let project = project_key();

    let created1 = client
        .create_issue(&project, "Task", &e2e_summary("pagination-1"), None, None, None)
        .expect("create_issue (1) should succeed");
    let key1 = created1["key"].as_str().expect("key missing").to_string();
    let _guard1 = IssueGuard::new(&key1, creds.clone());

    let created2 = client
        .create_issue(&project, "Task", &e2e_summary("pagination-2"), None, None, None)
        .expect("create_issue (2) should succeed");
    let key2 = created2["key"].as_str().expect("key missing").to_string();
    let _guard2 = IssueGuard::new(&key2, creds);

    let jql = format!("issue in ({key1}, {key2}) ORDER BY id ASC");

    // Page 1 — must return 1 issue and a nextPageToken
    let page1 = client
        .search_issues(&jql, 1, None, Some("summary"))
        .expect("page 1 search should succeed");

    let issues_p1 = page1["issues"].as_array().expect("issues must be array");
    assert_eq!(issues_p1.len(), 1, "page 1 should return exactly 1 issue");

    let next_token = page1["nextPageToken"]
        .as_str()
        .expect("nextPageToken must be present — query must have 2 issues")
        .to_string();

    // Page 2 — must return the other issue
    let page2 = client
        .search_issues(&jql, 1, Some(&next_token), Some("summary"))
        .expect("page 2 search should succeed");

    let issues_p2 = page2["issues"].as_array().expect("issues must be array");
    assert_eq!(issues_p2.len(), 1, "page 2 should return exactly 1 issue");

    let key_p1 = issues_p1[0]["key"].as_str().expect("key missing in p1");
    let key_p2 = issues_p2[0]["key"].as_str().expect("key missing in p2");
    assert_ne!(key_p1, key_p2, "page 2 must return a different issue than page 1");
    assert_eq!(key_p1, key1, "page 1 should return the first issue (ORDER BY id ASC)");
    assert_eq!(key_p2, key2, "page 2 should return the second issue (ORDER BY id ASC)");
}

// ── Cleanup (recovery) ───────────────────────────────────────────────────────

/// Finds and deletes all issues with the `[jira-cli-e2e]` prefix in the target project.
///
/// Run this after a test run where some tests panicked before their guards were
/// set up, or to clean up any issues left over from a previous interrupted session:
///
/// ```sh
/// JIRA_E2E_PROJECT=KAN cargo test -p jira e2e_cleanup -- --ignored
/// ```
#[test]
#[ignore = "e2e: recovery — deletes all [jira-cli-e2e] issues in JIRA_E2E_PROJECT"]
fn e2e_cleanup() {
    let (client, _) = setup();
    let project = project_key();

    let jql = format!("project = {project} AND summary ~ \"{E2E_PREFIX}\" ORDER BY created ASC");

    // Paginate through all results and delete each one.
    let mut page_token: Option<String> = None;
    let mut deleted = 0usize;

    loop {
        let results = client
            .search_issues(&jql, 50, page_token.as_deref(), Some("summary"))
            .expect("cleanup search should succeed");

        let issues = results["issues"].as_array().expect("issues must be array");
        if issues.is_empty() {
            break;
        }

        for issue in issues {
            let key = issue["key"].as_str().expect("key missing");
            match client.delete_issue(key, true) {
                Ok(()) => {
                    deleted += 1;
                    println!("deleted {key}");
                }
                Err(e) => eprintln!("failed to delete {key}: {e}"),
            }
        }

        match results["nextPageToken"].as_str() {
            Some(token) => page_token = Some(token.to_string()),
            None => break,
        }
    }

    println!("e2e_cleanup: deleted {deleted} issue(s)");
}
