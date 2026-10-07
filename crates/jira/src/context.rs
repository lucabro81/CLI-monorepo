//! Shared setup helpers used across command handlers.
//!
//! - `config_dir` — resolves the XDG config directory (`$XDG_CONFIG_HOME` or
//!   `~/.config`). Used by every command that touches the filesystem.
//! - `load_app_config` / `load_oauth_config` — load and validate `app.json` from
//!   the config dir (whole file / one identity's section), mapping
//!   `OAuthConfigError` to `CliError`.
//! - `authenticated_client` — the standard sequence for commands that call the
//!   Jira API as an identity (service account, or a person with `--user <id>`):
//!   load its config section → load its credentials → refresh if expired → build client.
//!   Centralised here so each command handler calls one function instead of
//!   repeating the load/refresh/build chain.
//! - `print_json` — renders a `serde_json::Value` via `cli_fields::render_json`
//!   (see that crate for the `--select`/`--select-all` contract) and prints it.

use std::path::Path;

use crate::auth::{self, AppConfig, Identity, LoginError, OAuthConfig, OAuthConfigError};
use crate::client::{ClientError, JiraClient};
use crate::error::CliError;

/// XDG-style config directory (`$XDG_CONFIG_HOME` or `~/.config`), used on every platform
/// so dev machines and headless deployment targets share the same layout.
pub fn config_dir() -> Result<std::path::PathBuf, CliError> {
    if let Ok(xdg) = std::env::var("XDG_CONFIG_HOME") {
        return Ok(std::path::PathBuf::from(xdg));
    }
    dirs::home_dir()
        .map(|h| h.join(".config"))
        .ok_or(CliError::NoHomeDirectory)
}

/// Loads `app.json`, mapping its errors to `CliError`.
pub fn load_app_config() -> Result<AppConfig, CliError> {
    let path = auth::app_config_path(&config_dir()?);
    AppConfig::load(&path).map_err(|e| app_config_error(e, &path))
}

/// Loads the OAuth app of `identity` from `app.json`.
pub fn load_oauth_config(identity: &Identity) -> Result<OAuthConfig, CliError> {
    let path = auth::app_config_path(&config_dir()?);
    oauth_section(load_app_config()?, identity, &path)
}

pub(crate) fn app_config_error(error: OAuthConfigError, path: &Path) -> CliError {
    let path = path.display().to_string();
    match error {
        OAuthConfigError::NotFound(_) => CliError::AppConfigNotFound { path },
        OAuthConfigError::InvalidJson(reason) => CliError::AppConfigInvalid { path, reason },
        OAuthConfigError::LegacyFormat => CliError::AppConfigLegacy { path },
    }
}

/// `app`'s section for `identity`, or an error naming the `init` command that adds it.
pub(crate) fn oauth_section(app: AppConfig, identity: &Identity, path: &Path) -> Result<OAuthConfig, CliError> {
    let path_str = || path.display().to_string();
    match identity {
        Identity::Service => app.service.ok_or_else(|| CliError::ServiceAppMissing { path: path_str() }),
        Identity::User(id) => {
            app.user.ok_or_else(|| CliError::UserAppMissing { path: path_str(), id: id.to_string() })
        }
    }
}

/// The command that logs `identity` in again.
pub(crate) fn login_command(identity: &Identity) -> String {
    match identity {
        Identity::Service => "jira auth login".to_string(),
        Identity::User(id) => format!("jira auth login --user {id}"),
    }
}

/// Maps a failure to load or renew `identity`'s stored credentials, kept at `credentials_path`.
pub(crate) fn login_error_to_cli(error: LoginError, credentials_path: &Path, identity: &Identity) -> CliError {
    let login = login_command(identity);
    match error {
        // An unwritable file or folder: logging in again would not help.
        LoginError::SaveCredentials(_) => CliError::SaveCredentialsFailed {
            path: credentials_path.display().to_string(),
            reason: error.to_string(),
        },
        // A person's refresh token refused: only their new login helps. For the
        // service identity the refused grant is app.json's own credentials.
        LoginError::TokenRejected(reason) => match identity {
            Identity::Service => CliError::TokenRefreshFailed { reason, login },
            Identity::User(id) => CliError::UserLoginExpired { reason, id: id.to_string() },
        },
        LoginError::TokenExchange(reason) | LoginError::AccessibleResources(reason) => {
            CliError::TokenRefreshFailed { reason, login }
        }
        // The file exists but can't be read: a new login would not fix it.
        LoginError::Io(e) if e.kind() != std::io::ErrorKind::NotFound => CliError::IoError {
            reason: format!("could not read {}: {e}", credentials_path.display()),
        },
        LoginError::NoAccessibleResources => CliError::TokenRefreshFailed {
            reason: "no accessible Jira sites found for this account".to_string(),
            login,
        },
        _ => match identity {
            Identity::Service => CliError::NotAuthenticatedService,
            Identity::User(id) => CliError::NotAuthenticatedUser { id: id.to_string() },
        },
    }
}

/// Loads and auto-refreshes `identity`'s OAuth credentials, then builds an authenticated Jira client.
/// Returns a clear error if that identity is not logged in or its session has expired.
pub fn authenticated_client(identity: &Identity) -> Result<JiraClient, CliError> {
    let oauth_config = load_oauth_config(identity)?;
    let path = auth::credentials_path(&config_dir()?, identity);
    let credentials =
        auth::load_credentials(&oauth_config, &path, identity).map_err(|e| login_error_to_cli(e, &path, identity))?;
    Ok(JiraClient::new(&credentials))
}

/// Prints `value` as pretty-printed JSON to stdout according to `select`.
/// See `cli_fields::Select` — an omitted `--select`/`--select-all` results in
/// `CliError::Select` instead of printing, unless the caller passed `Select::All`.
pub fn print_json(value: &serde_json::Value, select: cli_fields::Select<'_>) -> Result<(), CliError> {
    let output = cli_fields::render_json(value, select)?;
    println!("{output}");
    Ok(())
}

/// Maps a [`ClientError`] (HTTP/network layer) to the user-facing [`CliError`].
/// Shared by every command handler that calls a `JiraClient` method.
pub fn client_error_to_cli(e: ClientError) -> CliError {
    match e {
        ClientError::Request(reason) => CliError::ApiRequestFailed { reason },
        ClientError::Status { status, body } => CliError::ApiError { status, body },
    }
}

#[cfg(test)]
#[path = "tests/context_tests.rs"]
mod tests;
