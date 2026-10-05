//! OAuth 2.0 authentication against a ZITADEL instance (Cloud or self-hosted).
//!
//! - **Service user, private key JWT** (`login_service_user`, default) —
//!   non-interactive: the CLI signs a JWT assertion (RFC 7523) with the service
//!   user's private key from `app.json` and exchanges it at the instance's token
//!   endpoint. ZITADEL's recommended service-account method. Issues no
//!   `refresh_token`: renewal signs a fresh assertion.
//! - **Human, authorization code + PKCE** (`login_user`, `auth login --user`) —
//!   opens the browser on the Native app's authorize URL, waits for the redirect
//!   on a loopback listener (both from `crates/oauth-user-login`), exchanges the code. Issues a `refresh_token`;
//!   renewal uses the refresh grant (`renew` dispatches on its presence, so a
//!   human session never silently turns into the service user).
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
#[derive(Debug, Clone, PartialEq, Eq)]
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
/// downloaded from the ZITADEL console (its `"type"` field is ignored here and
/// checked by `from_key_file`).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ServiceUserKey {
    /// Key id — sent as the JWT `kid` header.
    pub key_id: String,
    /// RSA private key, PEM.
    pub key: String,
    /// The service user's id — the JWT `iss` and `sub` claims.
    pub user_id: String,
}

#[derive(Debug, PartialEq, Eq, thiserror::Error)]
pub enum KeyFileError {
    #[error("not a valid JSON key file: {0}")]
    InvalidJson(String),
    #[error(
        "this is a key of type {0:?}, not a service user key (\"serviceaccount\"). In the console \
        create it under Users > Service Users > <user> > Keys > New (JSON), not on an application"
    )]
    WrongType(String),
    #[error("its private key is not a valid RSA PEM key: {0}")]
    InvalidPrivateKey(String),
}

impl ServiceUserKey {
    /// Parses and validates a key file downloaded from the console: valid JSON,
    /// `"type": "serviceaccount"`, and a usable RSA private key.
    pub fn from_key_file(json: &str) -> Result<Self, KeyFileError> {
        let value: serde_json::Value =
            serde_json::from_str(json).map_err(|e| KeyFileError::InvalidJson(e.to_string()))?;
        let key_type = value["type"].as_str().unwrap_or_default();
        if key_type != "serviceaccount" {
            return Err(KeyFileError::WrongType(key_type.to_string()));
        }
        let key: ServiceUserKey =
            serde_json::from_value(value).map_err(|e| KeyFileError::InvalidJson(e.to_string()))?;
        jsonwebtoken::EncodingKey::from_rsa_pem(key.key.as_bytes())
            .map_err(|e| KeyFileError::InvalidPrivateKey(e.to_string()))?;
        Ok(key)
    }
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
    #[error(
        "no Native app configured: app.json has no \"client_id\". Create a Native application \
        (PKCE, redirect http://localhost:8080/callback, refresh token enabled) in a ZITADEL project \
        and run: zitadel init --client-id <client-id>"
    )]
    NativeAppNotConfigured,
    // No retry command here: CliError::UserLoginFailed adds it.
    #[error("{0}")]
    CallbackListener(oauth_user_login::ListenerError),
    #[error("{0}")]
    Callback(oauth_user_login::WaitError),
    #[error(
        "ZITADEL issued no refresh token, so the session could not be renewed. In the console enable \
        \"Refresh Token\" in the Native application's token settings, then run: zitadel auth login --user"
    )]
    NoRefreshToken,
    #[error("credentials file is corrupted ({0}). Run: zitadel auth login")]
    InvalidCredentialsFile(String),
    /// Writing `credentials.json` failed (e.g. after a successful renewal) —
    /// distinct from `Io`, which only covers reading it.
    #[error("could not write credentials file: {0}")]
    SaveCredentials(String),
    /// Serializing/signing well-typed values — should never fire in practice.
    #[error("internal error: {0}")]
    Internal(String),
}

#[derive(Debug, Deserialize)]
struct TokenResponse {
    access_token: String,
    /// Present for the human login (with `offline_access`); absent for the
    /// JWT-bearer grant, and possibly on a refresh response (old one stays valid).
    #[serde(default)]
    refresh_token: Option<String>,
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

/// Runs the interactive human login: browser consent on the Native app,
/// loopback callback, PKCE code exchange.
pub fn login_user(config: &AppConfig) -> Result<Credentials, LoginError> {
    let verifier = oauth_user_login::generate_code_verifier();
    let state = oauth_user_login::generate_state();
    // Fails on a missing client_id before anything is opened or bound.
    let url = authorization_url(config, &oauth_user_login::code_challenge(&verifier), &state)?;
    let listener = oauth_user_login::bind_listener(endpoints::CALLBACK_LISTEN_ADDR)
        .map_err(LoginError::CallbackListener)?;

    eprintln!("Opening the browser for ZITADEL login. If it does not open, visit:\n{url}\n");
    let _ = webbrowser::open(&url);

    let params = oauth_user_login::wait_for_callback(&listener, endpoints::CALLBACK_PATH, &state)
        .map_err(LoginError::Callback)?;
    exchange_code(config, &params.code, &verifier)
}

pub(crate) fn authorization_url(
    config: &AppConfig,
    code_challenge: &str,
    state: &str,
) -> Result<String, LoginError> {
    let client_id = config.client_id.as_deref().ok_or(LoginError::NativeAppNotConfigured)?;
    let params = [
        ("client_id", client_id),
        ("redirect_uri", endpoints::REDIRECT_URI),
        ("response_type", "code"),
        ("scope", endpoints::USER_SCOPES),
        ("state", state),
        ("code_challenge", code_challenge),
        ("code_challenge_method", "S256"),
    ];
    let query = serde_urlencoded::to_string(params)
        .map_err(|e| LoginError::Internal(format!("failed to encode authorization URL: {e}")))?;
    Ok(format!("{}?{query}", endpoints::authorize_url(&config.instance_url)))
}

pub(crate) fn exchange_code(
    config: &AppConfig,
    code: &str,
    code_verifier: &str,
) -> Result<Credentials, LoginError> {
    let client_id = config.client_id.as_deref().ok_or(LoginError::NativeAppNotConfigured)?;
    let pairs = [
        ("grant_type", "authorization_code"),
        ("code", code),
        ("redirect_uri", endpoints::REDIRECT_URI),
        ("client_id", client_id),
        ("code_verifier", code_verifier),
    ];
    let token = request_token(&config.instance_url, &pairs)?;
    let refresh_token = token.refresh_token.ok_or(LoginError::NoRefreshToken)?;
    Ok(Credentials {
        access_token: token.access_token,
        refresh_token: Some(refresh_token),
        expires_at: now_unix() + token.expires_in,
    })
}

/// Exchanges the stored refresh token for a new access token. Keeps the old
/// refresh token if ZITADEL doesn't return a new one.
pub(crate) fn refresh(config: &AppConfig, credentials: &Credentials) -> Result<Credentials, LoginError> {
    let client_id = config.client_id.as_deref().ok_or(LoginError::NativeAppNotConfigured)?;
    let refresh_token = credentials.refresh_token.as_deref().ok_or_else(|| {
        LoginError::Internal("refresh() called on credentials without a refresh token".to_string())
    })?;
    let pairs = [
        ("grant_type", "refresh_token"),
        ("refresh_token", refresh_token),
        ("client_id", client_id),
    ];
    let token = request_token(&config.instance_url, &pairs)?;
    Ok(Credentials {
        access_token: token.access_token,
        refresh_token: Some(token.refresh_token.unwrap_or_else(|| refresh_token.to_string())),
        expires_at: now_unix() + token.expires_in,
    })
}

/// Renews credentials whose access token has expired (or is about to): a human
/// session (with a refresh token) via the refresh grant, the service user by
/// re-signing its JWT.
pub fn renew(config: &AppConfig, credentials: &Credentials) -> Result<Credentials, LoginError> {
    match credentials.refresh_token {
        Some(_) => refresh(config, credentials),
        None => login_service_user(config),
    }
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
        let renewed = renew(config, &credentials)?;
        save_credentials(path, &renewed)?;
        return Ok(renewed);
    }

    Ok(credentials)
}

/// Writes credentials as pretty JSON with owner-only permissions (they hold
/// bearer tokens), creating parent directories as needed.
pub fn save_credentials(path: &Path, credentials: &Credentials) -> Result<(), LoginError> {
    use std::io::Write;

    let write_failed = |e: std::io::Error| LoginError::SaveCredentials(e.to_string());
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(write_failed)?;
    }
    let json = serde_json::to_string_pretty(credentials)
        .map_err(|e| LoginError::Internal(format!("failed to serialize credentials: {e}")))?;

    let mut options = std::fs::OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
        // New files are created 0600. An existing file loses any group/other
        // access first (owner bits are kept: a read-only file stays read-only).
        options.mode(0o600);
        if let Ok(metadata) = std::fs::metadata(path) {
            let mode = metadata.permissions().mode();
            if mode & 0o077 != 0 {
                std::fs::set_permissions(path, std::fs::Permissions::from_mode(mode & 0o700))
                    .map_err(write_failed)?;
            }
        }
    }
    options
        .open(path)
        .and_then(|mut file| file.write_all(json.as_bytes()))
        .map_err(write_failed)
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

#[cfg(test)]
#[path = "tests/auth_user_tests.rs"]
mod user_tests;
