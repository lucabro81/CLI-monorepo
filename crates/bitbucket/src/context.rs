//! Shared setup helpers used across command handlers.
//!
//! - `config_dir` — resolves the XDG config directory (`$XDG_CONFIG_HOME` or
//!   `~/.config`). Used by every command that touches the filesystem.
//! - `load_oauth_config` — loads and validates `app.json` from the config dir,
//!   mapping `OAuthConfigError` to `CliError`.
//! - `authenticated_client` — the standard sequence for commands that call the
//!   Bitbucket API: load config -> load credentials -> renew if expired -> build client.
//! - `print_json` — renders a `serde_json::Value` via `cli_fields::render_json`
//!   (see that crate for the `--select`/`--select-all` contract) and prints it.

use crate::auth::{self, OAuthConfig, OAuthConfigError};
use crate::client::BitbucketClient;
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

/// Loads and auto-renews credentials, then builds an authenticated Bitbucket client.
/// Returns a clear error if the user is not logged in or renewal fails.
pub fn authenticated_client() -> Result<BitbucketClient, CliError> {
    let oauth_config = load_oauth_config()?;
    let path = auth::credentials_path(&config_dir()?);
    let credentials = auth::load_credentials(&oauth_config, &path).map_err(|e| match e {
        auth::LoginError::Io(_) => CliError::NotAuthenticated,
        auth::LoginError::TokenExchange(reason) | auth::LoginError::Internal(reason) => {
            CliError::TokenRefreshFailed { reason }
        }
    })?;
    Ok(BitbucketClient::new(&credentials))
}

/// Prints `value` as pretty-printed JSON to stdout according to `select`.
/// See `cli_fields::Select` — an omitted `--select`/`--select-all` results in
/// `CliError::Select` instead of printing, unless the caller passed `Select::All`.
pub fn print_json(value: &serde_json::Value, select: cli_fields::Select<'_>) -> Result<(), CliError> {
    let output = cli_fields::render_json(value, select)?;
    println!("{output}");
    Ok(())
}

/// Splits `workspace/repo_slug` into its two parts, rejecting any other shape.
/// Shared by command groups that take a `workspace/repo_slug` identifier (`repo`, `pr`).
pub fn split_repository(repository: &str) -> Result<(&str, &str), CliError> {
    match repository.split_once('/') {
        Some((workspace, repo_slug)) if !workspace.is_empty() && !repo_slug.is_empty() => {
            Ok((workspace, repo_slug))
        }
        _ => Err(CliError::InvalidRepository {
            value: repository.to_string(),
        }),
    }
}

#[cfg(test)]
#[path = "tests/context_tests.rs"]
mod tests;
