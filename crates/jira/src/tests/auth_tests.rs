#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::path::{Path, PathBuf};

use super::{app_config_path, credentials_path, SCOPES};

// `auth.rs` is now a thin wrapper over `atlassian_auth` (see BACKLOG.md's
// LIB-1) — the OAuth flows, PKCE helpers, and callback parsing it delegates
// to are covered by that crate's own test suite. These tests only guard the
// two things that are actually jira-specific: which config directory this
// crate's paths resolve under, and what scopes it requests.

#[test]
fn credentials_path_is_under_jira_cli_dir() {
    let path = credentials_path(Path::new("/home/user/.config"));

    assert_eq!(
        path,
        PathBuf::from("/home/user/.config/jira-cli/credentials.json")
    );
}

#[test]
fn app_config_path_is_under_jira_cli_dir() {
    let path = app_config_path(Path::new("/home/user/.config"));

    assert_eq!(
        path,
        PathBuf::from("/home/user/.config/jira-cli/app.json")
    );
}

#[test]
fn scopes_include_offline_access() {
    // offline_access is what makes 3LO logins issue a refresh_token — losing
    // it from SCOPES would silently break `auth login --user` renewal.
    assert!(SCOPES.contains("offline_access"));
}
