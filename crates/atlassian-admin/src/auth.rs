//! Static Organization API key configuration for the Atlassian Organization
//! Admin API.
//!
//! Unlike every other crate in this workspace, there is no OAuth exchange and
//! no separate `credentials.json` — `app.json`'s `api_key` *is* the finished
//! Bearer token, used directly on every request, with no expiry or renewal
//! to manage. See `crates/atlassian-admin/CLAUDE.md`'s "Auth design" section
//! for why this crate has no `auth login`/`auth whoami` commands.

use serde::Deserialize;
use std::path::{Path, PathBuf};

/// Static Organization API key + org id loaded from `app.json`.
/// Written once by hand (or via `init`); never modified by the CLI at runtime.
#[derive(Debug, PartialEq, Eq)]
pub struct AdminConfig {
    pub api_key: String,
    pub org_id: String,
}

impl AdminConfig {
    /// Parses `{"api_key": "...", "org_id": "..."}` from the contents of `app.json`.
    pub fn from_json(json: &str) -> Result<Self, AdminConfigError> {
        let app: AppCredentials =
            serde_json::from_str(json).map_err(|e| AdminConfigError::InvalidJson(e.to_string()))?;

        Ok(AdminConfig {
            api_key: app.api_key,
            org_id: app.org_id,
        })
    }

    /// Loads app credentials from `<config_dir>/atlassian-admin-cli/app.json`.
    pub fn load(path: &Path) -> Result<Self, AdminConfigError> {
        let raw = std::fs::read_to_string(path)
            .map_err(|_| AdminConfigError::NotFound(path.to_path_buf()))?;
        Self::from_json(&raw)
    }
}

#[derive(Debug, Deserialize)]
struct AppCredentials {
    api_key: String,
    org_id: String,
}

/// Path to the app credentials file: `<config_dir>/atlassian-admin-cli/app.json`.
pub fn app_config_path(config_dir: &Path) -> PathBuf {
    config_dir.join("atlassian-admin-cli").join("app.json")
}

#[derive(Debug, PartialEq, Eq)]
pub enum AdminConfigError {
    NotFound(PathBuf),
    InvalidJson(String),
}

impl std::fmt::Display for AdminConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AdminConfigError::NotFound(path) => {
                write!(f, "app credentials file not found at {}", path.display())
            }
            AdminConfigError::InvalidJson(msg) => {
                write!(f, "invalid app credentials file: {msg}")
            }
        }
    }
}

#[cfg(test)]
#[path = "tests/auth_tests.rs"]
mod tests;
