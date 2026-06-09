//! Shared setup helpers used across command handlers.
//!
//! - `config_dir` — resolves the XDG config directory (`$XDG_CONFIG_HOME` or
//!   `~/.config`). Used by every command that touches the filesystem.
//! - `load_oauth_config` — loads and validates `app.json` from the config dir,
//!   mapping `OAuthConfigError` to `CliError`.
//! - `authenticated_client` — the standard sequence for commands that call the
//!   Jira API: load config → load credentials → refresh if expired → build client.
//!   Centralised here so each command handler calls one function instead of
//!   repeating the load/refresh/build chain.
//! - `print_json` — serialises a `serde_json::Value` to stdout, applying
//!   `--select` field projection beforehand if any paths were requested.

use crate::auth::{self, OAuthConfig, OAuthConfigError};
use crate::client::JiraClient;
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

pub fn load_oauth_config() -> Result<OAuthConfig, CliError> {
    let config_dir = config_dir()?;
    let path = auth::app_config_path(&config_dir);
    OAuthConfig::load(&path).map_err(|e| match e {
        OAuthConfigError::NotFound(_) => CliError::AppConfigNotFound {
            path: path.display().to_string(),
        },
        OAuthConfigError::InvalidJson(reason) => CliError::AppConfigInvalid {
            path: path.display().to_string(),
            reason,
        },
    })
}

/// Loads and auto-refreshes OAuth credentials, then builds an authenticated Jira client.
/// Returns a clear error if the user is not logged in or the session has expired.
pub fn authenticated_client() -> Result<JiraClient, CliError> {
    let oauth_config = load_oauth_config()?;
    let path = auth::credentials_path(&config_dir()?);
    let credentials = auth::load_credentials(&oauth_config, &path).map_err(|e| {
        use crate::auth::LoginError;
        match e {
            LoginError::TokenExchange(reason) => CliError::TokenRefreshFailed { reason },
            LoginError::NoAccessibleResources => CliError::TokenRefreshFailed {
                reason: "no accessible Jira sites found for this account".to_string(),
            },
            _ => CliError::NotAuthenticated,
        }
    })?;
    Ok(JiraClient::new(&credentials))
}

/// Prints `value` as pretty-printed JSON to stdout.
/// If `fields` is non-empty, only the specified dot-notation paths are included.
pub fn print_json(value: &serde_json::Value, fields: &[&str]) -> Result<(), CliError> {
    let filtered = crate::fields::filter_fields(value.clone(), fields);
    let output =
        serde_json::to_string_pretty(&filtered).map_err(|e| CliError::JsonSerialize {
            reason: e.to_string(),
        })?;
    println!("{output}");
    Ok(())
}
