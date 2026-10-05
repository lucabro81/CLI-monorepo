//! Shared setup helpers used across command handlers.
//!
//! - `config_dir` — resolves the XDG config directory (`$XDG_CONFIG_HOME` or `~/.config`).
//! - `load_app_config` — loads and validates `app.json`, mapping `AppConfigError` to `CliError`.

use crate::auth::{self, AppConfig, AppConfigError};
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
