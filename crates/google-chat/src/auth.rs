//! OAuth 2.0 authentication infrastructure for Google Chat, supporting two grant types:
//!
//! - **Service account + domain-wide delegation** (`login_service_account`,
//!   default) — non-interactive: the CLI signs a JWT assertion (RFC 7523)
//!   with the service account's private key, impersonating a Workspace user
//!   (`sub` claim) via domain-wide delegation, and exchanges it for an access
//!   token. No browser, no human interaction. The expected mode for
//!   agent-driven usage. Issues no `refresh_token` — a fresh JWT is signed
//!   and exchanged on every renewal instead.
//! - **Authorization Code + PKCE** (`login`, `auth login --user`) —
//!   interactive consent flow for a human Google account: PKCE challenge
//!   generation, browser launch, one-shot local HTTP server for the
//!   callback, authorization code exchange. Issues a `refresh_token`
//!   (requested via `access_type=offline`).
//!
//! There is no tenant-resolution step (no Jira-style `cloud_id`): Chat API
//! calls are scoped directly by space resource name under the authenticated
//! identity (the impersonated user, or the human who logged in).
//!
//! Other layers:
//!
//! - **App identity** (`OAuthConfig`) — the static Google OAuth app
//!   credentials loaded from `app.json`: `client_id`/`client_secret` for the
//!   3LO flow, plus an optional `service_account` block
//!   (`ServiceAccountConfig`) for the domain-wide-delegation flow.
//! - **Session credentials** (`Credentials`) — the dynamic token set (access
//!   token, optional refresh token, expiry) persisted to `credentials.json`.
//!
//! `refresh` exchanges a refresh token for a new access token; for an
//! Internal-consent-screen Google app, refresh tokens don't rotate or expire
//! on a fixed schedule, so unlike Atlassian there's no rotate-on-every-use
//! concern. Credentials with no `refresh_token` (service account) are
//! renewed by re-running `login_service_account` instead — `renew`
//! dispatches between the two.
//!
//! Path helpers (`app_config_path`, `credentials_path`) and persistence
//! helpers (`save_credentials`, `load_credentials`) are kept here so every
//! caller uses the same paths and serialization format.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

use crate::endpoints;

/// Static OAuth 2.0 app identity loaded from `app.json`.
/// Written once by hand (or by `google-chat init`); never modified by the CLI at runtime.
#[derive(Debug, PartialEq, Eq)]
pub struct OAuthConfig {
    pub client_id: String,
    pub client_secret: String,
    pub redirect_uri: String,
    /// Present only if the domain-wide-delegation flow has been set up
    /// (`login_service_account` requires it; `login` does not use it).
    pub service_account: Option<ServiceAccountConfig>,
}

impl OAuthConfig {
    pub const SCOPES: &'static str = "https://www.googleapis.com/auth/chat.spaces.readonly \
        https://www.googleapis.com/auth/chat.messages.readonly \
        https://www.googleapis.com/auth/chat.messages.create \
        https://www.googleapis.com/auth/chat.messages \
        https://www.googleapis.com/auth/chat.memberships.readonly \
        https://www.googleapis.com/auth/pubsub";
    pub const REDIRECT_URI: &'static str = "http://localhost:8080/callback";

    /// Parses app credentials from the contents of `app.json`.
    pub fn from_json(json: &str) -> Result<Self, OAuthConfigError> {
        let app: AppCredentials =
            serde_json::from_str(json).map_err(|e| OAuthConfigError::InvalidJson(e.to_string()))?;

        Ok(OAuthConfig {
            client_id: app.client_id,
            client_secret: app.client_secret,
            redirect_uri: Self::REDIRECT_URI.to_string(),
            service_account: app.service_account,
        })
    }

    /// Loads app credentials from `<config_dir>/google-chat-cli/app.json`.
    pub fn load(path: &Path) -> Result<Self, OAuthConfigError> {
        let raw = std::fs::read_to_string(path)
            .map_err(|_| OAuthConfigError::NotFound(path.to_path_buf()))?;
        Self::from_json(&raw)
    }
}

/// Service account identity used for the domain-wide-delegation flow, as written
/// (or hand-copied) into the `service_account` object of `app.json`.
#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct ServiceAccountConfig {
    /// `client_email` from the downloaded Google Cloud service account key JSON.
    pub client_email: String,
    /// `private_key` (PEM, RS256) from the downloaded service account key JSON.
    pub private_key: String,
    /// Email of the Workspace user this CLI impersonates via domain-wide delegation
    /// (the "service user" account dedicated to this automation).
    pub impersonate_user: String,
}

#[derive(Debug, Deserialize)]
struct AppCredentials {
    client_id: String,
    client_secret: String,
    #[serde(default)]
    service_account: Option<ServiceAccountConfig>,
}

/// Path to the app credentials file: `<config_dir>/google-chat-cli/app.json`.
pub fn app_config_path(config_dir: &Path) -> PathBuf {
    config_dir.join("google-chat-cli").join("app.json")
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
    #[error("invalid OAuth callback: {0:?}")]
    Callback(CallbackError),
    #[error("OAuth state mismatch — possible CSRF attack, login aborted")]
    StateMismatch,
    #[error("token exchange failed: {0}")]
    TokenExchange(String),
    /// Covers both "service account not configured" and "JWT signing failed" —
    /// conditions the caller should fix in `app.json`, not network errors.
    #[error("internal error: {0}")]
    Internal(String),
}

#[derive(Debug, Deserialize)]
struct TokenResponse {
    access_token: String,
    /// Present on the authorization-code exchange (when requested via
    /// `access_type=offline`) and may be absent on a `refresh_token` grant
    /// response, in which case the previous refresh token stays valid.
    /// Always absent on the `jwt-bearer` (service account) grant.
    #[serde(default)]
    refresh_token: Option<String>,
    expires_in: u64,
}

/// Runs the full interactive OAuth 2.0 Authorization Code + PKCE login flow:
/// opens the browser, waits for the local callback, exchanges the code for tokens,
/// and returns the resulting credentials.
pub fn login(config: &OAuthConfig) -> Result<Credentials, LoginError> {
    let verifier = generate_code_verifier();
    let challenge = code_challenge(&verifier);
    let state = generate_state();

    let url = authorization_url(config, &challenge, &state)?;
    eprintln!("Opening browser for Google Chat authorization:\n{url}\n");
    let _ = webbrowser::open(&url);

    let params = wait_for_callback(&state)?;

    let token = exchange_code_for_token(config, &params.code, &verifier)?;
    let refresh_token = token.refresh_token.ok_or_else(|| {
        LoginError::Internal(
            "Google did not return a refresh_token on this authorization. \
             This usually happens when the account has already granted consent \
             without access_type=offline in a previous session — revoke access at \
             https://myaccount.google.com/permissions and run \
             `google-chat auth login --user` again."
                .to_string(),
        )
    })?;

    Ok(Credentials {
        access_token: token.access_token,
        refresh_token: Some(refresh_token),
        expires_at: now_unix() + token.expires_in,
    })
}

/// Runs the non-interactive domain-wide-delegation flow for a service account:
/// signs a JWT assertion impersonating `service_account.impersonate_user` and
/// exchanges it for an access token. No browser, no user interaction. The
/// returned credentials have no `refresh_token` — `load_credentials` renews an
/// expired token by re-running this flow with a freshly signed assertion.
pub fn login_service_account(config: &OAuthConfig) -> Result<Credentials, LoginError> {
    let service_account = config.service_account.as_ref().ok_or_else(|| {
        LoginError::Internal(
            "service account not configured: app.json has no \"service_account\" object. \
             Add {\"client_email\", \"private_key\", \"impersonate_user\"} to app.json, or run \
             `google-chat auth login --user` for the interactive flow instead."
                .to_string(),
        )
    })?;

    let assertion = build_assertion(service_account)?;
    let pairs = [
        ("grant_type", endpoints::JWT_BEARER_GRANT_TYPE),
        ("assertion", assertion.as_str()),
    ];

    let token = request_token(&pairs)?;

    Ok(Credentials {
        access_token: token.access_token,
        refresh_token: None,
        expires_at: now_unix() + token.expires_in,
    })
}

/// Claims for the JWT-bearer assertion (RFC 7523), kept as a pure function of
/// `(service_account, iat)` so the claim shape is unit-testable without signing.
#[derive(Debug, Serialize, PartialEq, Eq)]
struct JwtClaims {
    iss: String,
    scope: String,
    aud: String,
    exp: u64,
    iat: u64,
    sub: String,
}

fn jwt_claims(service_account: &ServiceAccountConfig, iat: u64) -> JwtClaims {
    JwtClaims {
        iss: service_account.client_email.clone(),
        scope: OAuthConfig::SCOPES.to_string(),
        aud: endpoints::GOOGLE_OAUTH_TOKEN_URL.to_string(),
        exp: iat + 3600,
        iat,
        sub: service_account.impersonate_user.clone(),
    }
}

/// Signs the JWT-bearer assertion with the service account's RS256 private key.
fn build_assertion(service_account: &ServiceAccountConfig) -> Result<String, LoginError> {
    use jsonwebtoken::{Algorithm, EncodingKey, Header};

    let claims = jwt_claims(service_account, now_unix());
    let key = EncodingKey::from_rsa_pem(service_account.private_key.as_bytes())
        .map_err(|e| LoginError::Internal(format!("invalid service account private_key: {e}")))?;

    jsonwebtoken::encode(&Header::new(Algorithm::RS256), &claims, &key)
        .map_err(|e| LoginError::Internal(format!("failed to sign JWT assertion: {e}")))
}

fn now_unix() -> u64 {
    // Fallback to 0 if the system clock predates the Unix epoch (should never happen
    // on a real machine, but avoids a panic — a 0 timestamp causes the token to be
    // treated as expired and refreshed on the next call, which is safe behavior).
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

/// Listens once on the redirect URI's address for the authorization callback,
/// validates the `state`, and replies with a small confirmation page.
fn wait_for_callback(expected_state: &str) -> Result<CallbackParams, LoginError> {
    use std::io::{BufRead, BufReader, Write};
    use std::net::TcpListener;

    let listener = TcpListener::bind("127.0.0.1:8080").map_err(LoginError::Io)?;
    let (stream, _) = listener.accept().map_err(LoginError::Io)?;

    let mut reader = BufReader::new(&stream);
    let mut request_line = String::new();
    reader
        .read_line(&mut request_line)
        .map_err(LoginError::Io)?;

    let params = parse_callback_request_line(request_line.trim_end())
        .map_err(LoginError::Callback)?;

    let mut stream = stream;
    let body = "<html><body>Login complete — you can close this window and return to the terminal.</body></html>";
    let response = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    let _ = stream.write_all(response.as_bytes());

    if params.state != expected_state {
        return Err(LoginError::StateMismatch);
    }

    Ok(params)
}

fn exchange_code_for_token(
    config: &OAuthConfig,
    code: &str,
    code_verifier: &str,
) -> Result<TokenResponse, LoginError> {
    let pairs = [
        ("grant_type", "authorization_code"),
        ("client_id", config.client_id.as_str()),
        ("client_secret", config.client_secret.as_str()),
        ("code", code),
        ("redirect_uri", config.redirect_uri.as_str()),
        ("code_verifier", code_verifier),
    ];

    request_token(&pairs)
}

/// Exchanges a refresh token for a new access token.
///
/// Requires `credentials.refresh_token` to be `Some`. Callers must check this first
/// (`load_credentials` does); credentials with no refresh token (service account)
/// must be renewed via `login_service_account` instead.
pub fn refresh(config: &OAuthConfig, credentials: &Credentials) -> Result<Credentials, LoginError> {
    let refresh_token = credentials.refresh_token.as_ref().ok_or_else(|| {
        LoginError::Internal(
            "refresh() called on credentials with no refresh_token (service-account \
             credentials cannot be refreshed this way — re-run `google-chat auth login`)"
                .to_string(),
        )
    })?;

    let pairs = [
        ("grant_type", "refresh_token"),
        ("client_id", config.client_id.as_str()),
        ("client_secret", config.client_secret.as_str()),
        ("refresh_token", refresh_token.as_str()),
    ];

    let token = request_token(&pairs)?;

    Ok(Credentials {
        access_token: token.access_token,
        // Google does not always return a new refresh_token on a refresh_token
        // grant; keep the existing one when it doesn't.
        refresh_token: Some(token.refresh_token.unwrap_or_else(|| refresh_token.clone())),
        expires_at: now_unix() + token.expires_in,
    })
}

fn request_token(pairs: &[(&str, &str)]) -> Result<TokenResponse, LoginError> {
    let response = reqwest::blocking::Client::new()
        .post(endpoints::GOOGLE_OAUTH_TOKEN_URL)
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
/// Credentials with a `refresh_token` (3LO) are renewed via `refresh`; credentials
/// with none (service account) are renewed by re-running `login_service_account`.
pub fn renew(config: &OAuthConfig, credentials: &Credentials) -> Result<Credentials, LoginError> {
    match &credentials.refresh_token {
        Some(_) => refresh(config, credentials),
        None => login_service_account(config),
    }
}

/// Loads credentials from disk, renewing them first if the access token has expired.
pub fn load_credentials(config: &OAuthConfig, path: &Path) -> Result<Credentials, LoginError> {
    let raw = std::fs::read_to_string(path).map_err(LoginError::Io)?;
    let credentials: Credentials =
        serde_json::from_str(&raw).map_err(|e| LoginError::TokenExchange(e.to_string()))?;

    if now_unix() + 60 >= credentials.expires_at {
        let renewed = renew(config, &credentials)?;
        save_credentials(path, &renewed)?;
        return Ok(renewed);
    }

    Ok(credentials)
}

/// Serialises credentials to JSON and writes them to `path`, creating parent directories as needed.
pub fn save_credentials(path: &Path, credentials: &Credentials) -> Result<(), LoginError> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(LoginError::Io)?;
    }
    let json = serde_json::to_string_pretty(credentials)
        .map_err(|e| LoginError::Internal(format!("failed to serialize credentials: {e}")))?;
    std::fs::write(path, json).map_err(LoginError::Io)
}

#[derive(Debug, PartialEq, Eq)]
pub struct CallbackParams {
    pub code: String,
    pub state: String,
}

#[derive(Debug, PartialEq, Eq)]
pub enum CallbackError {
    MalformedRequestLine,
    MissingParam(&'static str),
}

/// Dynamic session credentials persisted to `credentials.json`.
/// Fully managed by the CLI — never edit by hand. Refreshed transparently before expiry.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Credentials {
    pub access_token: String,
    /// `Some` for 3LO (human) logins, which can be renewed via `refresh`.
    /// `None` for service-account (domain-wide-delegation) logins, which are
    /// renewed by re-running `login_service_account`.
    #[serde(default)]
    pub refresh_token: Option<String>,
    /// Unix timestamp (seconds) after which the access token is no longer valid.
    pub expires_at: u64,
}

/// Generates a PKCE code verifier: a random URL-safe string (43-128 chars per RFC 7636).
pub fn generate_code_verifier() -> String {
    random_url_safe_string(64)
}

/// Derives the PKCE code challenge (S256 method): base64url(sha256(verifier)), no padding.
pub fn code_challenge(verifier: &str) -> String {
    use base64::Engine;
    use sha2::{Digest, Sha256};

    let digest = Sha256::digest(verifier.as_bytes());
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(digest)
}

/// Generates a random opaque state string used to protect against CSRF in the OAuth flow.
pub fn generate_state() -> String {
    random_url_safe_string(32)
}

fn random_url_safe_string(byte_len: usize) -> String {
    use base64::Engine;
    use rand::RngCore;

    let mut bytes = vec![0u8; byte_len];
    rand::thread_rng().fill_bytes(&mut bytes);
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes)
}

/// Builds the Google authorization URL the user must open in a browser.
/// `access_type=offline` is required for Google to issue a `refresh_token`.
pub fn authorization_url(
    config: &OAuthConfig,
    code_challenge: &str,
    state: &str,
) -> Result<String, LoginError> {
    let params = [
        ("client_id", config.client_id.as_str()),
        ("scope", OAuthConfig::SCOPES),
        ("redirect_uri", config.redirect_uri.as_str()),
        ("state", state),
        ("response_type", "code"),
        ("access_type", "offline"),
        ("prompt", "consent"),
        ("code_challenge", code_challenge),
        ("code_challenge_method", "S256"),
    ];

    let query = serde_urlencoded::to_string(params)
        .map_err(|e| LoginError::Internal(format!("failed to encode authorization URL: {e}")))?;
    Ok(format!("{}?{query}", endpoints::GOOGLE_OAUTH_AUTHORIZE_URL))
}

/// Parses the first line of the local callback HTTP request, e.g.
/// `GET /callback?code=XYZ&state=abc HTTP/1.1`, extracting `code` and `state`.
pub fn parse_callback_request_line(line: &str) -> Result<CallbackParams, CallbackError> {
    let mut parts = line.split_whitespace();
    let (Some(_method), Some(target), Some(_version)) = (parts.next(), parts.next(), parts.next())
    else {
        return Err(CallbackError::MalformedRequestLine);
    };

    let query = target
        .split_once('?')
        .map(|(_, query)| query)
        .ok_or(CallbackError::MalformedRequestLine)?;

    let pairs: std::collections::HashMap<String, String> =
        serde_urlencoded::from_str(query).map_err(|_| CallbackError::MalformedRequestLine)?;

    Ok(CallbackParams {
        code: pairs
            .get("code")
            .cloned()
            .ok_or(CallbackError::MissingParam("code"))?,
        state: pairs
            .get("state")
            .cloned()
            .ok_or(CallbackError::MissingParam("state"))?,
    })
}

/// Path to the local credentials file: `<config_dir>/google-chat-cli/credentials.json`.
pub fn credentials_path(config_dir: &Path) -> PathBuf {
    config_dir.join("google-chat-cli").join("credentials.json")
}

#[cfg(test)]
#[path = "tests/auth_tests.rs"]
mod tests;
