//! Thin OAuth wrapper fixing this crate's product-specific pieces (config
//! directory name, OAuth scopes) on top of the shared `atlassian_auth` crate,
//! which implements the actual OAuth 2.0 (3LO + PKCE / `client_credentials`)
//! flows, PKCE helpers, and `cloud_id` resolution — identical logic shared
//! with `jira` (same `auth.atlassian.com`/`api.atlassian.com` platform). See
//! root `CLAUDE.md`'s "Shared library: crates/atlassian-auth" for why this
//! was extracted.

use std::path::{Path, PathBuf};

pub use atlassian_auth::{Credentials, LoginError, OAuthConfig, OAuthConfigError};

const CLI_DIR: &str = "confluence-cli";

/// OAuth scopes requested by the 3LO authorization URL. `client_credentials`
/// has no `scope` parameter of its own — it inherits whatever scopes were
/// granted at credential-creation time (see this crate's CLAUDE.md).
///
/// Mixes classic and granular scopes because this crate's commands span both
/// Confluence REST API versions: the v2 page/space endpoints have no classic
/// scope equivalent (always need the granular form), while `auth whoami`
/// (`GET /wiki/rest/api/user/current`) and CQL search are v1-only and use
/// classic scopes. Not yet live-verified against a real 3LO app — see this
/// crate's CLAUDE.md "OAuth / auth design" section.
pub const SCOPES: &str =
    "read:page:confluence write:page:confluence read:space:confluence read:confluence-user search:confluence offline_access";

/// Path to the app credentials file: `<config_dir>/confluence-cli/app.json`.
pub fn app_config_path(config_dir: &Path) -> PathBuf {
    atlassian_auth::app_config_path(config_dir, CLI_DIR)
}

/// Path to the local credentials file: `<config_dir>/confluence-cli/credentials.json`.
pub fn credentials_path(config_dir: &Path) -> PathBuf {
    atlassian_auth::credentials_path(config_dir, CLI_DIR)
}

/// Runs the interactive OAuth 2.0 (3LO) + PKCE login flow, requesting this crate's [`SCOPES`].
pub fn login(config: &OAuthConfig) -> Result<Credentials, LoginError> {
    atlassian_auth::login(config, SCOPES)
}

pub fn login_client_credentials(config: &OAuthConfig) -> Result<Credentials, LoginError> {
    atlassian_auth::login_client_credentials(config)
}

pub fn renew(config: &OAuthConfig, credentials: &Credentials) -> Result<Credentials, LoginError> {
    atlassian_auth::renew(config, credentials)
}

pub fn load_credentials(config: &OAuthConfig, path: &Path) -> Result<Credentials, LoginError> {
    atlassian_auth::load_credentials(config, path)
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
