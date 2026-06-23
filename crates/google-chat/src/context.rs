//! Shared setup helpers used across command handlers.
//!
//! - `config_dir` — resolves the XDG config directory (`$XDG_CONFIG_HOME` or
//!   `~/.config`). Used by every command that touches the filesystem.
//! - `load_oauth_config` — loads and validates `app.json` from the config dir,
//!   mapping `OAuthConfigError` to `CliError`.

use crate::auth::{self, OAuthConfig, OAuthConfigError};
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
