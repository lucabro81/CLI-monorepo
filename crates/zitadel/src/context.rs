//! Shared setup helpers used across command handlers.
//!
//! - `config_dir` — resolves the XDG config directory (`$XDG_CONFIG_HOME` or `~/.config`).
//! - `load_app_config` — loads and validates `app.json`, mapping `AppConfigError` to `CliError`.
//! - `authenticated_client` — load config → load the selected identity's credentials
//!   (the service user, or the human with `--user`) → renew if expiring → build client.
//! - `login_error_to_cli` — maps credential-loading failures (incl. a failed save
//!   after renewal, which must not be reported as "log in again").
//! - `client_error_to_cli` — maps `ClientError` to an actionable `CliError` (401/403/404 hints).
//! - `print_json` — renders a value via `cli_fields::render_json` honoring `--select`.
//! - `search_query` / `CONTAINS_IGNORE_CASE` — the pagination block and text-match
//!   method shared by every v2 search/list request body.

use crate::auth::{self, AppConfig, AppConfigError, Identity, LoginError};
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

/// Builds a client for the configured instance acting as `identity`, renewing
/// its stored token first if needed.
pub fn authenticated_client(identity: Identity) -> Result<ZitadelClient, CliError> {
    let config = load_app_config()?;
    let path = auth::credentials_path(&config_dir()?, identity);
    let credentials =
        auth::load_credentials(&config, &path, identity).map_err(|e| login_error_to_cli(e, &path, identity))?;
    Ok(ZitadelClient::new(&config.instance_url, &credentials))
}

/// Maps a credential-loading failure: unreadable/corrupted file → not
/// authenticated; failed write after a renewal → save failure naming the file;
/// anything else happened while renewing → token refresh failure.
pub fn login_error_to_cli(error: LoginError, credentials_path: &std::path::Path, identity: Identity) -> CliError {
    let reason = error.to_string();
    match (error, identity) {
        (LoginError::Io(_) | LoginError::InvalidCredentialsFile(_) | LoginError::WrongIdentity(_), Identity::Service) => {
            CliError::NotAuthenticatedService { reason }
        }
        (LoginError::Io(_) | LoginError::InvalidCredentialsFile(_) | LoginError::WrongIdentity(_), Identity::User) => {
            CliError::NotAuthenticatedUser { reason }
        }
        (LoginError::SaveCredentials(reason), _) => CliError::SaveCredentialsFailed {
            path: credentials_path.display().to_string(),
            reason,
        },
        (_, Identity::Service) => CliError::TokenRenewalFailedService { reason },
        (_, Identity::User) => CliError::TokenRefreshFailedUser { reason },
    }
}

/// The command that logs `identity` in again.
pub(crate) fn login_command(identity: Identity) -> &'static str {
    match identity {
        Identity::Service => "zitadel auth login",
        Identity::User => "zitadel auth login --user",
    }
}

pub fn client_error_to_cli(error: ClientError) -> CliError {
    match error {
        ClientError::Request(reason) => CliError::ApiRequestFailed { reason },
        ClientError::Status { status: 401, body } => CliError::ApiUnauthorized { body },
        ClientError::Status { status: 403, body } => CliError::ApiForbidden { body },
        ClientError::Status { status: 404, body } => CliError::ApiNotFound { body },
        ClientError::Status { status, body } => CliError::ApiError { status, body },
    }
}

/// Text-match method used by every free-text search flag.
pub const CONTAINS_IGNORE_CASE: &str = "TEXT_QUERY_METHOD_CONTAINS_IGNORE_CASE";

/// The pagination/ordering block of a v2 search/list request body (`query` in the
/// user/organization services, `pagination` in the project service).
pub fn search_query(limit: u32, offset: u64) -> serde_json::Value {
    serde_json::json!({"offset": offset, "limit": limit, "asc": true})
}

/// Prints `value` as pretty JSON according to `select` (see `cli_fields::Select`).
pub fn print_json(value: &serde_json::Value, select: cli_fields::Select<'_>) -> Result<(), CliError> {
    println!("{}", cli_fields::render_json(value, select)?);
    Ok(())
}

#[cfg(test)]
#[path = "tests/context_tests.rs"]
mod tests;
