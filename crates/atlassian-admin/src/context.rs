//! Shared setup helpers used across command handlers.
//!
//! - `config_dir` — resolves the XDG config directory (`$XDG_CONFIG_HOME` or
//!   `~/.config`). Used by every command that touches the filesystem.
//! - `load_admin_config` — loads and validates `app.json` from the config
//!   dir, mapping `AdminConfigError` to `CliError`.
//! - `authenticated_client` — builds an `AdminClient` directly from the
//!   static app config. Unlike other crates, there is no credential
//!   loading/renewal step: the API key from `app.json` is used as-is.
//! - `print_json` — renders a `serde_json::Value` via `cli_fields::render_json`
//!   (see that crate for the `--select`/`--select-all` contract) and prints it.

use crate::auth::{self, AdminConfig, AdminConfigError};
use crate::client::AdminClient;
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

pub fn load_admin_config() -> Result<AdminConfig, CliError> {
    let config_dir = config_dir()?;
    let path = auth::app_config_path(&config_dir);
    AdminConfig::load(&path).map_err(|e| match e {
        AdminConfigError::NotFound(_) => CliError::AppConfigNotFound {
            path: path.display().to_string(),
        },
        AdminConfigError::InvalidJson(reason) => CliError::AppConfigInvalid {
            path: path.display().to_string(),
            reason,
        },
    })
}

/// Builds an authenticated `AdminClient` directly from `app.json`. There is no
/// credential loading/renewal step here — the API key is a finished,
/// long-lived credential, not a short-lived token exchanged via OAuth.
pub fn authenticated_client() -> Result<AdminClient, CliError> {
    let config = load_admin_config()?;
    Ok(AdminClient::new(&config))
}

/// Prints `value` as pretty-printed JSON to stdout according to `select`.
/// See `cli_fields::Select` — an omitted `--select`/`--select-all` results in
/// `CliError::Select` instead of printing, unless the caller passed `Select::All`.
pub fn print_json(value: &serde_json::Value, select: cli_fields::Select<'_>) -> Result<(), CliError> {
    let output = cli_fields::render_json(value, select)?;
    println!("{output}");
    Ok(())
}

#[cfg(test)]
#[path = "tests/context_tests.rs"]
mod tests;
