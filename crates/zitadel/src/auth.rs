//! OAuth 2.0 authentication against a ZITADEL instance (Cloud or self-hosted).
//!
//! - **Service user, private key JWT** (`login_service_user`, default) —
//!   non-interactive: the CLI signs a JWT assertion (RFC 7523) with the service
//!   user's private key from `app.json` and exchanges it at the instance's token
//!   endpoint. ZITADEL's recommended service-account method. Issues no
//!   `refresh_token`: renewal signs a fresh assertion.
//!
//! Other layers:
//!
//! - **App configuration** (`AppConfig`) — `app.json`: the instance URL, the
//!   service user's key (the console key file, copied verbatim), and the Native
//!   app `client_id` used by the human login.
//! - **Session credentials** (`Credentials`) — `credentials.json`: access token,
//!   optional refresh token, expiry. Fully managed by the CLI.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

use crate::endpoints;

/// Static configuration loaded from `app.json`. Written by `zitadel init`;
/// never modified by the CLI at runtime.
#[derive(Debug, PartialEq, Eq)]
pub struct AppConfig {
    /// Instance base URL, normalized without trailing slashes
    /// (e.g. `https://acme.zitadel.cloud`). Also the JWT `aud` claim.
    pub instance_url: String,
    /// Needed by the default (service user) login.
    pub service_user: Option<ServiceUserKey>,
    /// Native app client id, needed by the human (`--user`) login.
    pub client_id: Option<String>,
}

/// A service user's private key, in the exact shape of the JSON key file
/// downloaded from the ZITADEL console (its `"type"` field is ignored).
#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ServiceUserKey {
    /// Key id — sent as the JWT `kid` header.
    pub key_id: String,
    /// RSA private key, PEM.
    pub key: String,
    /// The service user's id — the JWT `iss` and `sub` claims.
    pub user_id: String,
}

#[derive(Debug, Deserialize)]
struct RawAppConfig {
    instance_url: String,
    #[serde(default)]
    service_user: Option<ServiceUserKey>,
    #[serde(default)]
    client_id: Option<String>,
}

#[derive(Debug, PartialEq, Eq, thiserror::Error)]
pub enum AppConfigError {
    #[error("app config file not found at {}", .0.display())]
    NotFound(PathBuf),
    #[error("invalid app config file: {0}")]
    InvalidJson(String),
    #[error("invalid instance_url {0:?}: expected an absolute URL like https://<instance>.zitadel.cloud")]
    InvalidInstanceUrl(String),
}

impl AppConfig {
    /// Parses `app.json` contents, normalizing and validating `instance_url`.
    pub fn from_json(json: &str) -> Result<Self, AppConfigError> {
        let raw: RawAppConfig =
            serde_json::from_str(json).map_err(|e| AppConfigError::InvalidJson(e.to_string()))?;

        let instance_url = raw.instance_url.trim().trim_end_matches('/');
        let host = instance_url
            .strip_prefix("https://")
            .or_else(|| instance_url.strip_prefix("http://"));
        if host.is_none_or(str::is_empty) {
            return Err(AppConfigError::InvalidInstanceUrl(raw.instance_url));
        }

        Ok(AppConfig {
            instance_url: instance_url.to_string(),
            service_user: raw.service_user,
            client_id: raw.client_id,
        })
    }

    pub fn load(path: &Path) -> Result<Self, AppConfigError> {
        let raw = std::fs::read_to_string(path)
            .map_err(|_| AppConfigError::NotFound(path.to_path_buf()))?;
        Self::from_json(&raw)
    }
}

/// `<config_dir>/zitadel-cli/app.json`
pub fn app_config_path(config_dir: &Path) -> PathBuf {
    config_dir.join("zitadel-cli").join("app.json")
}

/// `<config_dir>/zitadel-cli/credentials.json`
pub fn credentials_path(config_dir: &Path) -> PathBuf {
    config_dir.join("zitadel-cli").join("credentials.json")
}

#[derive(Debug, thiserror::Error)]
pub enum LoginError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("token exchange failed: {0}")]
    TokenExchange(String),
    #[error(
        "no service user configured: app.json has no \"service_user\" key. \
        Download a JSON key for a ZITADEL service user from the console and run: \
        zitadel init --key-file <path-to-key.json>"
    )]
    ServiceUserNotConfigured,
    #[error(
        "the service user private key in app.json is not a valid RSA PEM key ({0}). \
        Download a new JSON key from the console and run: zitadel init --key-file <path-to-key.json>"
    )]
    InvalidPrivateKey(String),
    #[error("credentials file is corrupted ({0}). Run: zitadel auth login")]
    InvalidCredentialsFile(String),
    /// Serializing/signing well-typed values — should never fire in practice.
    #[error("internal error: {0}")]
    Internal(String),
}

#[derive(Debug, Deserialize)]
struct TokenResponse {
    access_token: String,
    expires_in: u64,
}

/// Dynamic session credentials persisted to `credentials.json`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Credentials {
    pub access_token: String,
    /// `None` for the service user (renewed by re-signing a JWT).
    #[serde(default)]
    pub refresh_token: Option<String>,
    /// Unix timestamp (seconds) after which the access token is no longer valid.
    pub expires_at: u64,
}

/// Signs a fresh JWT assertion with the service user's key and exchanges it
/// for an access token. No browser, no human interaction.
pub fn login_service_user(config: &AppConfig) -> Result<Credentials, LoginError> {
    let key = config
        .service_user
        .as_ref()
        .ok_or(LoginError::ServiceUserNotConfigured)?;

    let assertion = build_assertion(key, &config.instance_url, now_unix())?;
    let pairs = [
        ("grant_type", endpoints::JWT_BEARER_GRANT_TYPE),
        ("scope", endpoints::SERVICE_USER_SCOPES),
        ("assertion", assertion.as_str()),
    ];
    let token = request_token(&config.instance_url, &pairs)?;

    Ok(Credentials {
        access_token: token.access_token,
        refresh_token: None,
        expires_at: now_unix() + token.expires_in,
    })
}

/// JWT claims per ZITADEL's private key JWT spec, a pure function of its inputs
/// so the claim shape is unit-testable without signing.
#[derive(Debug, Serialize, PartialEq, Eq)]
pub(crate) struct JwtClaims {
    pub iss: String,
    pub sub: String,
    pub aud: String,
    pub iat: u64,
    pub exp: u64,
}

pub(crate) fn jwt_claims(key: &ServiceUserKey, instance_url: &str, iat: u64) -> JwtClaims {
    JwtClaims {
        iss: key.user_id.clone(),
        sub: key.user_id.clone(),
        aud: instance_url.to_string(),
        iat,
        exp: iat + 3600,
    }
}

/// Signs the assertion: RS256, `kid` = the key id from the console key file.
pub(crate) fn build_assertion(
    key: &ServiceUserKey,
    instance_url: &str,
    iat: u64,
) -> Result<String, LoginError> {
    use jsonwebtoken::{Algorithm, EncodingKey, Header};

    let encoding_key = EncodingKey::from_rsa_pem(key.key.as_bytes())
        .map_err(|e| LoginError::InvalidPrivateKey(e.to_string()))?;
    let mut header = Header::new(Algorithm::RS256);
    header.kid = Some(key.key_id.clone());

    jsonwebtoken::encode(&header, &jwt_claims(key, instance_url, iat), &encoding_key)
        .map_err(|e| LoginError::Internal(format!("failed to sign JWT assertion: {e}")))
}

fn request_token(instance_url: &str, pairs: &[(&str, &str)]) -> Result<TokenResponse, LoginError> {
    let response = reqwest::blocking::Client::new()
        .post(endpoints::token_url(instance_url))
        .form(pairs)
        .send()
        .map_err(|e| LoginError::TokenExchange(e.to_string()))?;

    if !response.status().is_success() {
        let status = response.status();
        let text = response.text().unwrap_or_default();
        return Err(LoginError::TokenExchange(format!("{status}: {text}")));
    }

    response
        .json::<TokenResponse>()
        .map_err(|e| LoginError::TokenExchange(e.to_string()))
}

/// Renews credentials whose access token has expired (or is about to).
pub fn renew(config: &AppConfig) -> Result<Credentials, LoginError> {
    login_service_user(config)
}

/// True when the token expires within the next 60 seconds (or already has).
pub(crate) fn is_expiring(expires_at: u64, now: u64) -> bool {
    now + 60 >= expires_at
}

/// Loads credentials from disk, renewing and re-saving them first if expiring.
pub fn load_credentials(config: &AppConfig, path: &Path) -> Result<Credentials, LoginError> {
    let raw = std::fs::read_to_string(path)?;
    let credentials: Credentials = serde_json::from_str(&raw)
        .map_err(|e| LoginError::InvalidCredentialsFile(e.to_string()))?;

    if is_expiring(credentials.expires_at, now_unix()) {
        let renewed = renew(config)?;
        save_credentials(path, &renewed)?;
        return Ok(renewed);
    }

    Ok(credentials)
}

/// Writes credentials as pretty JSON, creating parent directories as needed.
pub fn save_credentials(path: &Path, credentials: &Credentials) -> Result<(), LoginError> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let json = serde_json::to_string_pretty(credentials)
        .map_err(|e| LoginError::Internal(format!("failed to serialize credentials: {e}")))?;
    std::fs::write(path, json)?;
    Ok(())
}

fn now_unix() -> u64 {
    // A pre-epoch clock (never on a real machine) yields 0, which only makes the
    // token look expired and triggers a renewal — safe, and avoids a panic.
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

#[cfg(test)]
#[path = "tests/auth_tests.rs"]
mod tests;
