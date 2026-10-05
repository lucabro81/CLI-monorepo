//! Shared setup helpers used across command handlers.
//!
//! - `config_dir` — resolves the XDG config directory (`$XDG_CONFIG_HOME` or `~/.config`).
//! - `load_app_config` — loads and validates `app.json`, mapping `AppConfigError` to `CliError`.
//! - `authenticated_client` — load config → load credentials → renew if expiring → build client.
//! - `client_error_to_cli` — maps `ClientError` to an actionable `CliError` (401/403 hints).
//! - `print_json` — renders a value via `cli_fields::render_json` honoring `--select`.

use crate::auth::{self, AppConfig, AppConfigError, LoginError};
use crate::client::{ClientError, ZitadelClient};
use crate::error::CliError;

/// XDG-style config directory (`$XDG_CONFIG_HOME` or `~/.config`), used on every platform.
pub fn config_dir() -> Result<std::path::PathBuf, CliError> {
    if let Ok(xdg) = std::env::var("XDG_CONFIG_HOME") {
        return Ok(std::path::PathBuf::from(xdg));
    }
    dirs::home_dir()
        .map(|h| h.join(".config"))
        .ok_or(CliError::NoHomeDirectory)
}

pub fn load_app_config() -> Result<AppConfig, CliError> {
    let path = auth::app_config_path(&config_dir()?);
    AppConfig::load(&path).map_err(|e| match e {
        AppConfigError::NotFound(_) => CliError::AppConfigNotFound {
            path: path.display().to_string(),
        },
        AppConfigError::InvalidJson(_) | AppConfigError::InvalidInstanceUrl(_) => {
            CliError::AppConfigInvalid {
                path: path.display().to_string(),
                reason: e.to_string(),
            }
        }
    })
}

/// Builds a client for the configured instance, renewing the stored token first if needed.
pub fn authenticated_client() -> Result<ZitadelClient, CliError> {
    let config = load_app_config()?;
    let path = auth::credentials_path(&config_dir()?);
    let credentials = auth::load_credentials(&config, &path).map_err(|e| match e {
        LoginError::Io(_) | LoginError::InvalidCredentialsFile(_) => CliError::NotAuthenticated {
            reason: e.to_string(),
        },
        _ => CliError::TokenRefreshFailed {
            reason: e.to_string(),
        },
    })?;
    Ok(ZitadelClient::new(&config.instance_url, &credentials))
}

pub fn client_error_to_cli(error: ClientError) -> CliError {
    match error {
        ClientError::Request(reason) => CliError::ApiRequestFailed { reason },
        ClientError::Status { status: 401, body } => CliError::ApiUnauthorized { body },
        ClientError::Status { status: 403, body } => CliError::ApiForbidden { body },
        ClientError::Status { status, body } => CliError::ApiError { status, body },
    }
}

/// Prints `value` as pretty JSON according to `select` (see `cli_fields::Select`).
pub fn print_json(value: &serde_json::Value, select: cli_fields::Select<'_>) -> Result<(), CliError> {
    println!("{}", cli_fields::render_json(value, select)?);
    Ok(())
}

#[cfg(test)]
#[path = "tests/context_tests.rs"]
mod tests;
