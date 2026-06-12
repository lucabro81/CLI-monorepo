//! End-to-end tests against a real Bitbucket Cloud workspace.
//!
//! # Prerequisites
//!
//! - `bitbucket auth login` must have been run on this machine.
//! - `git` must be installed and on `PATH`.
//! - The OAuth consumer must have `repository:write`, `repository:admin`,
//!   and `pullrequest:write` scopes.
//! - Optional: `BITBUCKET_E2E_WORKSPACE` (defaults to `lucabrognaracode`) —
//!   must be a workspace where the authenticated account can create and
//!   delete repositories.
//!
//! # Running
//!
//! ```sh
//! cargo test -p bitbucket -- --ignored --test-threads=1
//! ```
//!
//! Run a single test:
//! ```sh
//! cargo test -p bitbucket e2e_pr_lifecycle -- --ignored
//! ```
//!
//! # Isolation
//!
//! Every repo created by these tests is named `cli-bitbucket-e2e-<timestamp>`.
//! `RepoGuard` deletes the repo on drop (even on test failure or panic). If a
//! guard is skipped due to a panic before it is created, run `e2e_cleanup` to
//! delete orphaned `cli-bitbucket-e2e-*` repos.
//!
//! Tests are not safe to run concurrently against the same workspace (each
//! creates and deletes its own repo, but `--test-threads=1` keeps runs simple
//! to reason about and avoids hammering the API in parallel).

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::path::Path;
use std::process::Command;

use serde_json::json;

use crate::auth::{self, Credentials};
use crate::client::BitbucketClient;
use crate::context;

const E2E_PREFIX: &str = "cli-bitbucket-e2e";

// ── Helpers ─────────────────────────────────────────────────────────────────

/// Returns the workspace slug to run e2e tests against.
fn workspace() -> String {
    std::env::var("BITBUCKET_E2E_WORKSPACE").unwrap_or_else(|_| "lucabrognaracode".to_string())
}

/// Builds an authenticated `BitbucketClient` and returns the credentials alongside it
/// so they can be stored in a `RepoGuard` and reused for `git` over HTTPS.
fn setup() -> (BitbucketClient, Credentials) {
    let oauth_config =
        context::load_oauth_config().expect("app.json not found — run `bitbucket init` first");
    let config_dir = context::config_dir().expect("could not resolve config dir");
    let path = auth::credentials_path(&config_dir);
    let credentials = auth::load_credentials(&oauth_config, &path)
        .expect("not authenticated — run `bitbucket auth login` first");
    let client = BitbucketClient::new(&credentials);
    (client, credentials)
}

/// Builds a unique repo slug with a timestamp suffix.
fn e2e_repo_slug(label: &str) -> String {
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    format!("{E2E_PREFIX}-{label}-{ts}")
}

/// Runs `git` with the given args in `cwd`, panicking with stderr on failure.
fn git(cwd: &Path, args: &[&str]) {
    let output = Command::new("git")
        .args(args)
        .current_dir(cwd)
        .env("GIT_TERMINAL_PROMPT", "0")
        .output()
        .expect("failed to run git — is it installed and on PATH?");

    assert!(
        output.status.success(),
        "git {args:?} failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

// ── RepoGuard ───────────────────────────────────────────────────────────────

/// RAII guard that deletes a Bitbucket repository on drop.
///
/// Deletion is best-effort: errors are silently ignored since we cannot
/// propagate errors from `Drop`.
struct RepoGuard {
    workspace: String,
    repo_slug: String,
    credentials: Credentials,
}

impl RepoGuard {
    fn new(workspace: impl Into<String>, repo_slug: impl Into<String>, credentials: Credentials) -> Self {
        Self {
            workspace: workspace.into(),
            repo_slug: repo_slug.into(),
            credentials,
        }
    }
}

impl Drop for RepoGuard {
    fn drop(&mut self) {
        let client = BitbucketClient::new(&self.credentials);
        let _ = client.delete_repository(&self.workspace, &self.repo_slug);
    }
}

// ── PR lifecycle ────────────────────────────────────────────────────────────

#[test]
#[ignore = "e2e: requires credentials, git, and a writable Bitbucket workspace"]
#[allow(clippy::too_many_lines)]
fn e2e_pr_lifecycle() {
    let (client, creds) = setup();
    let ws = workspace();
    let repo_slug = e2e_repo_slug("pr");

    // Create throwaway repo
    client
        .create_repository(&ws, &repo_slug, &json!({"scm": "git", "is_private": true}))
        .expect("create_repository should succeed");
    let _guard = RepoGuard::new(&ws, &repo_slug, creds.clone());

    // Set up a local clone with two feature branches, pushed over HTTPS using
    // the OAuth access token as a bearer ("x-token-auth").
    let tmp = tempfile::tempdir().expect("tempdir");
    let dir = tmp.path();
    let remote_url = format!(
        "https://x-token-auth:{}@bitbucket.org/{ws}/{repo_slug}.git",
        creds.access_token
    );

    git(dir, &["-c", "init.defaultBranch=main", "init"]);
    git(dir, &["config", "user.email", "cli-e2e@example.com"]);
    git(dir, &["config", "user.name", "bitbucket-cli e2e"]);
    std::fs::write(dir.join("file.txt"), "line1\n").expect("write file.txt");
    git(dir, &["add", "file.txt"]);
    git(dir, &["commit", "-m", "initial commit"]);
    git(dir, &["remote", "add", "origin", &remote_url]);
    git(dir, &["push", "-u", "origin", "main"]);

    // feature-merge: appends a line, used for the approve/comment/merge path
    git(dir, &["checkout", "-b", "feature-merge"]);
    std::fs::write(dir.join("file.txt"), "line1\nline2\n").expect("write file.txt");
    git(dir, &["commit", "-am", "add line2"]);
    git(dir, &["push", "-u", "origin", "feature-merge"]);

    // feature-decline: a second branch off main, used for the decline path
    git(dir, &["checkout", "main"]);
    git(dir, &["checkout", "-b", "feature-decline"]);
    std::fs::write(dir.join("file.txt"), "line1\nline0\n").expect("write file.txt");
    git(dir, &["commit", "-am", "prepend line0"]);
    git(dir, &["push", "-u", "origin", "feature-decline"]);

    // branch list: all three branches present
    let branches = client
        .list_branches(&ws, &repo_slug, None)
        .expect("list_branches should succeed");
    let branch_names: Vec<&str> = branches["values"]
        .as_array()
        .expect("values must be an array")
        .iter()
        .map(|b| b["name"].as_str().expect("branch name missing"))
        .collect();
    assert!(branch_names.contains(&"main"), "main branch missing: {branch_names:?}");
    assert!(
        branch_names.contains(&"feature-merge"),
        "feature-merge branch missing: {branch_names:?}"
    );
    assert!(
        branch_names.contains(&"feature-decline"),
        "feature-decline branch missing: {branch_names:?}"
    );

    // pr create (merge path)
    let created = client
        .create_pull_request(
            &ws,
            &repo_slug,
            &json!({
                "title": "[cli-bitbucket-e2e] merge path",
                "source": {"branch": {"name": "feature-merge"}},
                "destination": {"branch": {"name": "main"}},
            }),
        )
        .expect("create_pull_request should succeed");
    let pr_id = created["id"].as_u64().expect("pull request id missing");

    // pr get: title, state, source branch
    let pr = client
        .get_pull_request(&ws, &repo_slug, pr_id)
        .expect("get_pull_request should succeed");
    assert_eq!(pr["title"], "[cli-bitbucket-e2e] merge path");
    assert_eq!(pr["state"], "OPEN");
    assert_eq!(pr["source"]["branch"]["name"], "feature-merge");

    // pr list --state OPEN: our PR is present
    let open_prs = client
        .list_pull_requests(&ws, &repo_slug, Some("OPEN"), None)
        .expect("list_pull_requests should succeed");
    let found = open_prs["values"]
        .as_array()
        .expect("values must be an array")
        .iter()
        .any(|p| p["id"].as_u64() == Some(pr_id));
    assert!(found, "pr {pr_id} not found in OPEN pull requests");

    // pr comment: general
    client
        .create_pull_request_comment(&ws, &repo_slug, pr_id, &json!({"content": {"raw": "general comment"}}))
        .expect("general comment should succeed");

    // pr comment: inline on file.txt line 2
    client
        .create_pull_request_comment(
            &ws,
            &repo_slug,
            pr_id,
            &json!({
                "content": {"raw": "inline comment"},
                "inline": {"path": "file.txt", "to": 2},
            }),
        )
        .expect("inline comment should succeed");

    // pr approve: response is the participant entry
    let approval = client
        .approve_pull_request(&ws, &repo_slug, pr_id)
        .expect("approve_pull_request should succeed");
    assert_eq!(approval["approved"], true);

    // pr unapprove
    client
        .unapprove_pull_request(&ws, &repo_slug, pr_id)
        .expect("unapprove_pull_request should succeed");

    // pr merge --confirm
    let merged = client
        .merge_pull_request(&ws, &repo_slug, pr_id, &json!({}))
        .expect("merge_pull_request should succeed");
    assert_eq!(merged["state"], "MERGED");

    // pr create (decline path)
    let created_decline = client
        .create_pull_request(
            &ws,
            &repo_slug,
            &json!({
                "title": "[cli-bitbucket-e2e] decline path",
                "source": {"branch": {"name": "feature-decline"}},
                "destination": {"branch": {"name": "main"}},
            }),
        )
        .expect("create_pull_request (decline path) should succeed");
    let decline_pr_id = created_decline["id"].as_u64().expect("pull request id missing");

    // pr decline --confirm
    let declined = client
        .decline_pull_request(&ws, &repo_slug, decline_pr_id)
        .expect("decline_pull_request should succeed");
    assert_eq!(declined["state"], "DECLINED");

    // _guard drops here, deleting the repo
}

// ── Cleanup (recovery) ──────────────────────────────────────────────────────

/// Finds and deletes all repositories with the `cli-bitbucket-e2e-` prefix in
/// the target workspace.
///
/// Run this after a test run where a panic occurred before a `RepoGuard` was
/// set up, or to clean up any repos left over from a previous interrupted
/// session:
///
/// ```sh
/// cargo test -p bitbucket e2e_cleanup -- --ignored
/// ```
#[test]
#[ignore = "e2e: recovery — deletes all cli-bitbucket-e2e-* repos in the target workspace"]
fn e2e_cleanup() {
    let (client, _) = setup();
    let ws = workspace();

    let mut page = 1;
    let mut deleted = 0usize;

    loop {
        let results = client
            .list_repositories(&ws, Some(page))
            .expect("cleanup list_repositories should succeed");

        let values = results["values"].as_array().expect("values must be an array");
        if values.is_empty() {
            break;
        }

        for repo in values {
            let slug = repo["slug"].as_str().expect("slug missing");
            if slug.starts_with(E2E_PREFIX) {
                match client.delete_repository(&ws, slug) {
                    Ok(()) => {
                        deleted += 1;
                        println!("deleted {slug}");
                    }
                    Err(e) => eprintln!("failed to delete {slug}: {e}"),
                }
            }
        }

        if results.get("next").is_none() {
            break;
        }
        page += 1;
    }

    println!("e2e_cleanup: deleted {deleted} repo(s)");
}
