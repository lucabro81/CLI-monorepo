//! OAuth 2.0 `client_credentials` authentication for a Bitbucket OAuth consumer.
//!
//! Bitbucket's native OAuth consumers (workspace Settings → OAuth consumers,
//! created without a callback URL) support the `client_credentials` grant:
//! the consumer's `client_id`/`client_secret` are exchanged directly for an
//! access token via HTTP Basic auth, no browser or user interaction. The
//! resulting token has no `refresh_token` — it is renewed by re-running the
//! same exchange.
//!
//! - **App identity** (`OAuthConfig`) — the static OAuth consumer credentials
//!   loaded from `app.json`.
//! - **Session credentials** (`Credentials`) — the access token and its
//!   expiry, persisted to `credentials.json`.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

use crate::endpoints;

/// Static OAuth consumer identity loaded from `app.json`.
/// Written once by hand; never modified by the CLI at runtime.
#[derive(Debug, PartialEq, Eq)]
pub struct OAuthConfig {
    pub client_id: String,
    pub client_secret: String,
}

impl OAuthConfig {
    /// Parses app credentials (`client_id`, `client_secret`) from the contents of `app.json`.
    pub fn from_json(json: &str) -> Result<Self, OAuthConfigError> {
        let app: AppCredentials =
            serde_json::from_str(json).map_err(|e| OAuthConfigError::InvalidJson(e.to_string()))?;

        Ok(OAuthConfig {
            client_id: app.client_id,
            client_secret: app.client_secret,
        })
    }

    /// Loads app credentials from `<config_dir>/bitbucket-cli/app.json`.
    pub fn load(path: &Path) -> Result<Self, OAuthConfigError> {
        let raw = std::fs::read_to_string(path)
            .map_err(|_| OAuthConfigError::NotFound(path.to_path_buf()))?;
        Self::from_json(&raw)
    }
}

#[derive(Debug, Deserialize)]
struct AppCredentials {
    client_id: String,
    client_secret: String,
}

/// Path to the app credentials file: `<config_dir>/bitbucket-cli/app.json`.
pub fn app_config_path(config_dir: &Path) -> PathBuf {
    config_dir.join("bitbucket-cli").join("app.json")
}

/// Path to the local credentials file: `<config_dir>/bitbucket-cli/credentials.json`.
pub fn credentials_path(config_dir: &Path) -> PathBuf {
    config_dir.join("bitbucket-cli").join("credentials.json")
}

#[derive(Debug, PartialEq, Eq)]
pub enum OAuthConfigError {
    NotFound(PathBuf),
    InvalidJson(String),
}

impl std::fmt::Display for OAuthConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            OAuthConfigError::NotFound(path) => {
                write!(f, "app credentials file not found at {}", path.display())
            }
            OAuthConfigError::InvalidJson(msg) => {
                write!(f, "invalid app credentials file: {msg}")
            }
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum LoginError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("token exchange failed: {0}")]
    TokenExchange(String),
    /// A condition that should be unreachable given valid inputs.
    /// If this surfaces it indicates a bug in the CLI itself.
    #[error("internal error: {0}")]
    Internal(String),
}

#[derive(Debug, Deserialize)]
struct TokenResponse {
    access_token: String,
    expires_in: u64,
    /// Space-separated list of OAuth scopes granted to the consumer, e.g.
    /// "repository:read pullrequest:write account:read". Bitbucket's
    /// `client_credentials` response uses the standard `OAuth2` (RFC 6749)
    /// field name "scope" (singular), not "scopes".
    scope: String,
}

/// Dynamic session credentials persisted to `credentials.json`.
/// Fully managed by the CLI — never edit by hand. Renewed transparently before expiry.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Credentials {
    pub access_token: String,
    /// Unix timestamp (seconds) after which the access token is no longer valid.
    pub expires_at: u64,
    /// OAuth scopes granted to the consumer, as returned by the token endpoint.
    /// Used by `doctor` to report which commands are likely to work without
    /// an extra API call.
    pub scopes: Vec<String>,
}

fn now_unix() -> u64 {
    // Fallback to 0 if the system clock predates the Unix epoch (should never happen
    // on a real machine, but avoids a panic — a 0 timestamp causes the token to be
    // treated as expired and renewed on the next call, which is safe behavior).
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

/// Runs the OAuth 2.0 `client_credentials` flow: exchanges the OAuth consumer's
/// `client_id`/`client_secret` (via HTTP Basic auth) for an access token. No
/// browser, no user interaction, no refresh token.
pub fn login_client_credentials(config: &OAuthConfig) -> Result<Credentials, LoginError> {
    let response = reqwest::blocking::Client::new()
        .post(endpoints::BITBUCKET_TOKEN_URL)
        .basic_auth(&config.client_id, Some(&config.client_secret))
        .form(&[("grant_type", "client_credentials")])
        .send()
        .map_err(|e| LoginError::TokenExchange(e.to_string()))?;

    if !response.status().is_success() {
        let status = response.status();
        let text = response.text().unwrap_or_default();
        return Err(LoginError::TokenExchange(format!("{status}: {text}")));
    }

    let token: TokenResponse = response
        .json()
        .map_err(|e| LoginError::TokenExchange(e.to_string()))?;

    Ok(Credentials {
        access_token: token.access_token,
        expires_at: now_unix() + token.expires_in,
        scopes: token.scope.split_whitespace().map(str::to_string).collect(),
    })
}

/// Loads credentials from disk, renewing them first if the access token has
/// expired (or is about to, within 60s).
pub fn load_credentials(config: &OAuthConfig, path: &Path) -> Result<Credentials, LoginError> {
    let raw = std::fs::read_to_string(path)?;
    let credentials: Credentials =
        serde_json::from_str(&raw).map_err(|e| LoginError::TokenExchange(e.to_string()))?;

    if now_unix() + 60 >= credentials.expires_at {
        let renewed = login_client_credentials(config)?;
        save_credentials(path, &renewed)?;
        return Ok(renewed);
    }

    Ok(credentials)
}

/// Serialises credentials to JSON and writes them to `path`, creating parent directories as needed.
pub fn save_credentials(path: &Path, credentials: &Credentials) -> Result<(), LoginError> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let json = serde_json::to_string_pretty(credentials)
        .map_err(|e| LoginError::Internal(format!("failed to serialize credentials: {e}")))?;
    std::fs::write(path, json)?;
    Ok(())
}

#[cfg(test)]
#[path = "tests/auth_tests.rs"]
mod tests;
