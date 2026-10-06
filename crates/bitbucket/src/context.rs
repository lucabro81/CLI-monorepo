//! Shared setup helpers used across command handlers.
//!
//! - `config_dir` — resolves the XDG config directory (`$XDG_CONFIG_HOME` or
//!   `~/.config`). Used by every command that touches the filesystem.
//! - `load_app_config` / `load_oauth_config` — load and validate `app.json` from
//!   the config dir (whole file / one identity's section), mapping
//!   `OAuthConfigError` to `CliError`.
//! - `authenticated_client` — the standard sequence for commands that call the
//!   Bitbucket API as an identity (the OAuth app, or a person with `--user <id>`):
//!   load its config section -> load its credentials -> renew if expired -> build client.
//! - `print_json` — renders a `serde_json::Value` via `cli_fields::render_json`
//!   (see that crate for the `--select`/`--select-all` contract) and prints it.

use std::path::Path;

use crate::auth::{self, AppConfig, Identity, LoginError, OAuthConfig, OAuthConfigError};
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

/// Loads `app.json`, mapping its errors to `CliError`.
pub fn load_app_config() -> Result<AppConfig, CliError> {
    let path = auth::app_config_path(&config_dir()?);
    AppConfig::load(&path).map_err(|e| app_config_error(e, &path))
}

/// Loads the OAuth consumer of `identity` from `app.json`.
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
        Identity::Service => "bitbucket auth login".to_string(),
        Identity::User(id) => format!("bitbucket auth login --user {id}"),
    }
}

/// Maps a failure to load or renew `identity`'s stored credentials.
pub(crate) fn login_error_to_cli(error: LoginError, identity: &Identity) -> CliError {
    let reason = match error {
        LoginError::Io(_) | LoginError::WrongIdentity(_) => {
            return match identity {
                Identity::Service => CliError::NotAuthenticatedService,
                Identity::User(id) => CliError::NotAuthenticatedUser { id: id.to_string() },
            };
        }
        LoginError::TokenExchange(reason) | LoginError::Internal(reason) => reason,
        // Login-only failures (callback/listener, remote login); unreachable
        // while renewing, mapped anyway rather than panicking.
        other @ (LoginError::CallbackListener(_) | LoginError::Callback(_) | LoginError::PendingLogin(_)) => {
            other.to_string()
        }
    };
    match identity {
        Identity::Service => CliError::TokenRenewalFailedService { reason },
        Identity::User(id) => CliError::TokenRefreshFailedUser { reason, id: id.to_string() },
    }
}

/// Loads and auto-renews `identity`'s credentials, then builds an authenticated Bitbucket client.
/// Returns a clear error if that identity is not logged in or renewal fails.
pub fn authenticated_client(identity: &Identity) -> Result<BitbucketClient, CliError> {
    let oauth_config = load_oauth_config(identity)?;
    let path = auth::credentials_path(&config_dir()?, identity);
    let credentials =
        auth::load_credentials(&oauth_config, &path, identity).map_err(|e| login_error_to_cli(e, identity))?;
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
