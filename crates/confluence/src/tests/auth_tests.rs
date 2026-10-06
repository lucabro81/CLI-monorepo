#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::path::{Path, PathBuf};

use atlassian_auth::Identity;
use oauth_user_login::UserId;

use super::{app_config_path, credentials_path, pending_login_path, SCOPES};

// `auth.rs` is a thin wrapper over `atlassian_auth` — the OAuth flows (local
// and remote) it delegates to are covered by that crate's own test suite, PKCE
// and the callback listener by `oauth_user_login`'s. These tests only guard the two things that are actually
// confluence-specific: which config directory this crate's paths resolve
// under, and what scopes it requests.

#[test]
fn each_identity_has_its_own_credentials_file_under_confluence_cli_dir() {
    let config_dir = Path::new("/home/user/.config");

    assert_eq!(
        credentials_path(config_dir, &Identity::Service),
        PathBuf::from("/home/user/.config/confluence-cli/credentials-service.json")
    );
    assert_eq!(
        credentials_path(config_dir, &Identity::User(UserId::parse("alice").unwrap())),
        PathBuf::from("/home/user/.config/confluence-cli/users/alice/credentials.json")
    );
}

#[test]
fn app_config_path_is_under_confluence_cli_dir() {
    let path = app_config_path(Path::new("/home/user/.config"));

    assert_eq!(
        path,
        PathBuf::from("/home/user/.config/confluence-cli/app.json")
    );
}

#[test]
fn scopes_include_offline_access() {
    // offline_access is what makes 3LO logins issue a refresh_token — losing
    // it from SCOPES would silently break `auth login --user` renewal.
    assert!(SCOPES.contains("offline_access"));
}

#[test]
fn each_persons_pending_login_is_under_confluence_cli_dir() {
    assert_eq!(
        pending_login_path(Path::new("/cfg"), &UserId::parse("alice").unwrap()),
        PathBuf::from("/cfg/confluence-cli/users/alice/pending-login.json")
    );
}
