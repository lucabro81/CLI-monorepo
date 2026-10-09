//! Thin OAuth wrapper fixing this crate's product-specific pieces (config
//! directory name, OAuth scopes) on top of the shared `atlassian_auth` crate,
//! which implements the actual OAuth 2.0 (3LO + PKCE / `client_credentials`)
//! flows, PKCE helpers, and `cloud_id` resolution — identical logic shared
//! with `confluence`. See root `CLAUDE.md`'s "Shared library:
//! crates/atlassian-auth" for why this was extracted, and this crate's own
//! `CLAUDE.md` "OAuth / auth design" section for the two grant types and
//! their tradeoffs.

use std::path::{Path, PathBuf};

pub use atlassian_auth::{AppConfig, Credentials, Identity, LoginError, OAuthConfig, OAuthConfigError};
pub use oauth_user_login::UserId;

const CLI_DIR: &str = "jira-cli";

/// OAuth scopes requested by the 3LO authorization URL. `client_credentials`
/// has no `scope` parameter of its own — it inherits whatever scopes were
/// granted at credential-creation time (see this crate's CLAUDE.md).
pub const SCOPES: &str = "read:jira-work read:jira-user write:jira-work offline_access";

/// Path to the app credentials file: `<config_dir>/jira-cli/app.json`.
pub fn app_config_path(config_dir: &Path) -> PathBuf {
    atlassian_auth::app_config_path(config_dir, CLI_DIR)
}

/// Path to `identity`'s credentials file:
/// `<config_dir>/jira-cli/credentials-service.json` or `users/<id>/credentials.json`.
pub fn credentials_path(config_dir: &Path, identity: &Identity) -> PathBuf {
    identity.credentials_path(config_dir, CLI_DIR)
}

/// Path to a person's pending remote login: `<config_dir>/jira-cli/users/<id>/pending-login.json`.
pub fn pending_login_path(config_dir: &Path, id: &UserId) -> PathBuf {
    oauth_user_login::pending_login_path(config_dir, CLI_DIR, id)
}

/// The people logged in, for `doctor`.
pub fn list_users(config_dir: &Path) -> std::io::Result<Vec<UserId>> {
    oauth_user_login::list_users(config_dir, CLI_DIR)
}

/// Credentials files of earlier layouts still present (never read), for `doctor`.
pub fn legacy_credentials_files(config_dir: &Path) -> Vec<&'static str> {
    oauth_user_login::legacy_credentials_files(config_dir, CLI_DIR)
}

/// Removes `identity`'s stored login (`auth logout`); `false` when there was none.
pub fn remove_identity(config_dir: &Path, identity: &Identity) -> std::io::Result<bool> {
    oauth_user_login::remove_identity(config_dir, CLI_DIR, identity)
}

/// Step 1 of `auth login --user <id> --remote`, requesting this crate's [`SCOPES`].
pub fn start_remote_login(
    config: &OAuthConfig,
    redirect_uri: &str,
    pending_path: &Path,
) -> Result<(String, oauth_user_login::PendingLogin), LoginError> {
    atlassian_auth::start_remote_login(config, SCOPES, redirect_uri, pending_path, atlassian_auth::now_unix())
}

/// Step 2 of `auth login --user <id> --remote` (`--code --state`).
pub fn complete_remote_login(
    config: &OAuthConfig,
    pending_path: &Path,
    code: &str,
    state: &str,
) -> Result<Credentials, LoginError> {
    atlassian_auth::complete_remote_login(config, pending_path, code, state, atlassian_auth::now_unix())
}

/// Runs the interactive OAuth 2.0 (3LO) + PKCE login flow, requesting this crate's [`SCOPES`].
pub fn login(config: &OAuthConfig) -> Result<Credentials, LoginError> {
    atlassian_auth::login(config, SCOPES)
}

pub fn login_client_credentials(config: &OAuthConfig) -> Result<Credentials, LoginError> {
    atlassian_auth::login_client_credentials(config)
}

pub fn load_credentials(config: &OAuthConfig, path: &Path, identity: &Identity) -> Result<Credentials, LoginError> {
    atlassian_auth::load_credentials(config, path, identity)
}

/// Renews `identity`'s credentials after the API answered 401 to `rejected_token` (issue #240).
pub fn renew_rejected(
    config: &OAuthConfig,
    path: &Path,
    identity: &Identity,
    rejected_token: &str,
) -> Result<Credentials, LoginError> {
    atlassian_auth::renew_rejected(config, path, identity, rejected_token)
}

pub fn save_credentials(path: &Path, credentials: &Credentials) -> Result<(), LoginError> {
    atlassian_auth::save_credentials(path, credentials)
}

/// Fetches the OAuth scopes actually granted to `access_token`, used by `doctor`'s
/// `oauth_scopes` check.
pub fn get_granted_scopes(access_token: &str, cloud_id: &str) -> Result<Vec<String>, LoginError> {
    atlassian_auth::get_granted_scopes(access_token, cloud_id)
}

#[cfg(test)]
#[path = "tests/auth_tests.rs"]
mod tests;
