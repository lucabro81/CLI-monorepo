//! OAuth 2.0 authentication against a ZITADEL instance (Cloud or self-hosted).
//!
//! - **Service user, private key JWT** (`login_service_user`, default) —
//!   non-interactive: the CLI signs a JWT assertion (RFC 7523) with the service
//!   user's private key from `app.json` and exchanges it at the instance's token
//!   endpoint. ZITADEL's recommended service-account method. Issues no
//!   `refresh_token`: renewal signs a fresh assertion.
//! - **Human, authorization code + PKCE** (`login_user`, `auth login --user <id>`) —
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
//! - **Session credentials** (`Credentials`) — the identity's credentials file
//!   (`credentials-service.json` / `users/<id>/credentials.json`): access token,
//!   optional refresh token, expiry. Fully managed by the CLI, renewed under a
//!   per-file lock.

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
    /// Native app client id, needed by the human (`--user <id>`) login, shared by every person.
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
    config_dir.join(CLI_DIR).join("app.json")
}

pub use oauth_user_login::{Identity, UserId};

const CLI_DIR: &str = "zitadel-cli";

/// `identity`'s credentials file:
/// `<config_dir>/zitadel-cli/credentials-service.json` or `users/<id>/credentials.json`.
pub fn credentials_path(config_dir: &Path, identity: &Identity) -> PathBuf {
    identity.credentials_path(config_dir, CLI_DIR)
}

/// `<config_dir>/zitadel-cli/users/<id>/pending-login.json`, a person's state
/// between the two steps of `auth login --user <id> --remote`.
pub fn pending_login_path(config_dir: &Path, id: &UserId) -> PathBuf {
    oauth_user_login::pending_login_path(config_dir, CLI_DIR, id)
}

/// The people logged in, for `doctor` and `init`.
pub fn list_users(config_dir: &Path) -> std::io::Result<Vec<UserId>> {
    oauth_user_login::list_users(config_dir, CLI_DIR)
}

/// Credentials files of earlier layouts still present (never read), for `doctor`.
pub fn legacy_credentials_files(config_dir: &Path) -> Vec<&'static str> {
    oauth_user_login::legacy_credentials_files(config_dir, CLI_DIR)
}

/// Removes `identity`'s stored login (`auth logout`, `init` on a new instance);
/// `false` when there was none.
pub fn remove_identity(config_dir: &Path, identity: &Identity) -> std::io::Result<bool> {
    oauth_user_login::remove_identity(config_dir, CLI_DIR, identity)
}

#[derive(Debug, thiserror::Error)]
pub enum LoginError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("token exchange failed: {0}")]
    TokenExchange(String),
    /// The token endpoint refused the grant (HTTP 400/401/403, e.g. an expired
    /// or revoked refresh token): only a new login helps (issue #194).
    /// `TokenExchange` covers transient failures, where retrying may work.
    #[error("token request rejected: {0}")]
    TokenRejected(String),
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
    #[error("{0}")]
    PendingLogin(oauth_user_login::PendingLoginError),
    #[error(
        "ZITADEL issued no refresh token, so the session could not be renewed. In the console enable \
        \"Refresh Token\" in the Native application's token settings, then run: zitadel auth login --user <USER_ID>"
    )]
    NoRefreshToken,
    #[error("credentials file is corrupted ({0})")]
    InvalidCredentialsFile(String),
    /// The credentials file doesn't hold the identity it is named after.
    #[error("{0}")]
    WrongIdentity(&'static str),
    /// Writing the credentials file failed (e.g. after a successful renewal) —
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

/// Dynamic session credentials persisted to the identity's credentials file
/// (`credentials-service.json` / `users/<id>/credentials.json`).
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
        let detail = format!("{status}: {text}");
        return Err(if oauth_user_login::token_request_rejected(status.as_u16(), &text) {
            LoginError::TokenRejected(detail)
        } else {
            LoginError::TokenExchange(detail)
        });
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
    let url = authorization_url(
        config,
        &oauth_user_login::code_challenge(&verifier),
        &state,
        endpoints::REDIRECT_URI,
    )?;
    let listener = oauth_user_login::bind_listener(endpoints::CALLBACK_LISTEN_ADDR)
        .map_err(LoginError::CallbackListener)?;

    eprintln!("Opening the browser for ZITADEL login. If it does not open, visit:\n{url}\n");
    let _ = webbrowser::open(&url);

    let params = oauth_user_login::wait_for_callback(&listener, endpoints::CALLBACK_PATH, &state)
        .map_err(LoginError::Callback)?;
    exchange_code(config, &params.code, &verifier, endpoints::REDIRECT_URI)
}

/// Step 1 of the two-step (`--remote`) login: saves a pending login at `path`
/// and returns the authorize URL to hand to the person. No browser, no port.
pub fn start_remote_login(
    config: &AppConfig,
    redirect_uri: &str,
    path: &Path,
    now: u64,
) -> Result<(String, oauth_user_login::PendingLogin), LoginError> {
    let pending = oauth_user_login::PendingLogin::new(Some(redirect_uri), true, now);
    let challenge = pending
        .code_challenge()
        .ok_or_else(|| LoginError::Internal("a PKCE pending login has no verifier".to_string()))?;
    // Fails on a missing client_id before anything is saved.
    let url = authorization_url(config, &challenge, &pending.state, redirect_uri)?;
    pending.save(path).map_err(LoginError::PendingLogin)?;
    Ok((url, pending))
}

/// Step 2: takes the pending login for `state` (single-use) and exchanges
/// `code` with its stored verifier and redirect URI.
pub fn complete_remote_login(
    config: &AppConfig,
    path: &Path,
    code: &str,
    state: &str,
    now: u64,
) -> Result<Credentials, LoginError> {
    let pending =
        oauth_user_login::take_pending_login(path, state, now).map_err(LoginError::PendingLogin)?;
    let verifier = pending
        .code_verifier
        .as_deref()
        .ok_or_else(|| LoginError::Internal("the pending login has no PKCE verifier".to_string()))?;
    let redirect_uri = pending
        .redirect_uri
        .as_deref()
        .ok_or_else(|| LoginError::Internal("the pending login has no redirect URI".to_string()))?;
    exchange_code(config, code, verifier, redirect_uri)
}

pub(crate) fn authorization_url(
    config: &AppConfig,
    code_challenge: &str,
    state: &str,
    redirect_uri: &str,
) -> Result<String, LoginError> {
    let client_id = config.client_id.as_deref().ok_or(LoginError::NativeAppNotConfigured)?;
    let params = [
        ("client_id", client_id),
        ("redirect_uri", redirect_uri),
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
    redirect_uri: &str,
) -> Result<Credentials, LoginError> {
    let client_id = config.client_id.as_deref().ok_or(LoginError::NativeAppNotConfigured)?;
    let pairs = [
        ("grant_type", "authorization_code"),
        ("code", code),
        ("redirect_uri", redirect_uri),
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

/// Refuses credentials that don't belong to `identity`. Renewal picks its
/// grant from the stored token (refresh token, or a re-signed service user
/// JWT), so without this check a human slot with no refresh token would be
/// renewed as the service user, and a service slot holding a human login as
/// that human.
pub fn check_identity(credentials: &Credentials, identity: &Identity) -> Result<(), LoginError> {
    match (identity, credentials.refresh_token.is_some()) {
        (Identity::User(_), false) => Err(LoginError::WrongIdentity(
            "the human credentials file holds no refresh token, so it is not a human login",
        )),
        (Identity::Service, true) => Err(LoginError::WrongIdentity(
            "the service user credentials file holds a refresh token, so it is a human login",
        )),
        _ => Ok(()),
    }
}

/// Loads credentials from disk, renewing and re-saving them first if expiring.
pub fn load_credentials(config: &AppConfig, path: &Path, identity: &Identity) -> Result<Credentials, LoginError> {
    load_credentials_with(path, identity, |credentials| renew(config, credentials))
}

/// [`load_credentials`] with the renewal injected, so tests can count and fake it.
///
/// Renewal runs under an exclusive lock on the file (issue #175): two calls
/// for the same identity may both find the token expired, and must not both
/// spend the same refresh token. After taking the lock the file is read again,
/// and a token renewed meanwhile is returned as is.
pub(crate) fn load_credentials_with(
    path: &Path,
    identity: &Identity,
    renew: impl FnOnce(&Credentials) -> Result<Credentials, LoginError>,
) -> Result<Credentials, LoginError> {
    let credentials = read_credentials(path, identity)?;
    if !is_expiring(credentials.expires_at, now_unix()) {
        return Ok(credentials);
    }

    // Failing to create the lock file means the folder is not writable: the
    // same problem as failing to save, not a missing login.
    let _lock = oauth_user_login::lock_exclusive(path).map_err(|e| LoginError::SaveCredentials(e.to_string()))?;
    let credentials = read_credentials(path, identity)?;
    if !is_expiring(credentials.expires_at, now_unix()) {
        return Ok(credentials);
    }
    let renewed = renew(&credentials)?;
    save_credentials(path, &renewed)?;
    Ok(renewed)
}

/// Renews credentials whose access token ZITADEL answered 401 to, although it
/// had not expired (revoked, e.g. the person ended their session; issue #240).
pub fn renew_rejected(
    config: &AppConfig,
    path: &Path,
    identity: &Identity,
    rejected_token: &str,
) -> Result<Credentials, LoginError> {
    renew_rejected_with(path, identity, rejected_token, |credentials| renew(config, credentials))
}

/// [`renew_rejected`] with the renewal injected. Same lock as
/// [`load_credentials_with`]: if the file no longer holds `rejected_token`,
/// another call renewed it meanwhile and its token is returned as is.
pub(crate) fn renew_rejected_with(
    path: &Path,
    identity: &Identity,
    rejected_token: &str,
    renew: impl FnOnce(&Credentials) -> Result<Credentials, LoginError>,
) -> Result<Credentials, LoginError> {
    // Read before locking, so a login removed meanwhile is reported as missing
    // instead of having its folder recreated for the lock file.
    read_credentials(path, identity)?;
    let _lock = oauth_user_login::lock_exclusive(path).map_err(|e| LoginError::SaveCredentials(e.to_string()))?;
    let credentials = read_credentials(path, identity)?;
    if credentials.access_token != rejected_token {
        return Ok(credentials);
    }
    let renewed = renew(&credentials)?;
    save_credentials(path, &renewed)?;
    Ok(renewed)
}

pub(crate) fn read_credentials(path: &Path, identity: &Identity) -> Result<Credentials, LoginError> {
    let raw = std::fs::read_to_string(path)?;
    let credentials: Credentials = serde_json::from_str(&raw)
        .map_err(|e| LoginError::InvalidCredentialsFile(e.to_string()))?;
    check_identity(&credentials, identity)?;
    Ok(credentials)
}

/// Writes credentials as pretty JSON, atomically and with owner-only
/// permissions (they hold bearer tokens), creating parent directories as needed.
pub fn save_credentials(path: &Path, credentials: &Credentials) -> Result<(), LoginError> {
    let json = serde_json::to_string_pretty(credentials)
        .map_err(|e| LoginError::Internal(format!("failed to serialize credentials: {e}")))?;
    oauth_user_login::write_secret_file(path, json.as_bytes()).map_err(|e| LoginError::SaveCredentials(e.to_string()))
}

pub(crate) fn now_unix() -> u64 {
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
