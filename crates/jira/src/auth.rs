//! OAuth 2.0 authentication infrastructure, supporting two grant types:
//!
//! - **3LO + PKCE** (`login`) — interactive consent flow for a human Atlassian
//!   account: PKCE challenge generation, browser launch, one-shot local HTTP
//!   server for the callback, authorization code exchange, and cloud ID
//!   resolution via the accessible-resources endpoint. Issues a `refresh_token`.
//! - **`client_credentials`** (`login_client_credentials`) — non-interactive flow
//!   for a service account: exchanges `client_id`/`client_secret` directly for
//!   an access token, no browser involved. Issues no `refresh_token`.
//!
//! Other layers:
//!
//! - **App identity** (`OAuthConfig`) — the static Atlassian OAuth app credentials
//!   loaded from `app.json`. Includes helpers for loading and validating the file.
//! - **Session credentials** (`Credentials`) — the dynamic token set (access token,
//!   optional refresh token, expiry, cloud ID) persisted to `credentials.json`.
//!
//! `refresh` exchanges a refresh token for a new token pair. Atlassian refresh
//! tokens **rotate on every use** — the new pair must always be persisted immediately
//! to avoid invalidating the stored token. Credentials with no `refresh_token`
//! (service accounts) are renewed by re-running `login_client_credentials` instead.
//!
//! Path helpers (`app_config_path`, `credentials_path`) and persistence helpers
//! (`save_credentials`, `load_credentials`) are kept here so every caller uses the
//! same paths and serialization format.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Static OAuth 2.0 app identity loaded from `app.json`.
/// Written once by hand (or by `jira init`); never modified by the CLI at runtime.
#[derive(Debug, PartialEq, Eq)]
pub struct OAuthConfig {
    pub client_id: String,
    pub client_secret: String,
    pub redirect_uri: String,
}

impl OAuthConfig {
    pub const SCOPES: &'static str =
        "read:jira-work read:jira-user write:jira-work offline_access";
    pub const REDIRECT_URI: &'static str = "http://localhost:8080/callback";

    /// Parses app credentials (`client_id`, `client_secret`) from the contents of `app.json`.
    pub fn from_json(json: &str) -> Result<Self, OAuthConfigError> {
        let app: AppCredentials =
            serde_json::from_str(json).map_err(|e| OAuthConfigError::InvalidJson(e.to_string()))?;

        Ok(OAuthConfig {
            client_id: app.client_id,
            client_secret: app.client_secret,
            redirect_uri: Self::REDIRECT_URI.to_string(),
        })
    }

    /// Loads app credentials from `<config_dir>/jira-cli/app.json`.
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

/// Path to the app credentials file: `<config_dir>/jira-cli/app.json`.
pub fn app_config_path(config_dir: &Path) -> PathBuf {
    config_dir.join("jira-cli").join("app.json")
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
    #[error("no accessible Jira sites found for this account")]
    NoAccessibleResources,
    /// A condition that should be unreachable given valid inputs.
    /// If this surfaces it indicates a bug in the CLI itself.
    #[error("internal error: {0}")]
    Internal(String),
}

#[derive(Debug, Deserialize)]
struct TokenResponse {
    access_token: String,
    /// Present for the `authorization_code` and `refresh_token` grants; absent
    /// for `client_credentials` (service accounts get no refresh token).
    #[serde(default)]
    refresh_token: Option<String>,
    expires_in: u64,
}

#[derive(Debug, Deserialize)]
struct AccessibleResource {
    id: String,
}

/// Runs the full interactive OAuth 2.0 (3LO) + PKCE login flow:
/// opens the browser, waits for the local callback, exchanges the code for tokens,
/// resolves the Jira cloud id, and returns the resulting credentials.
pub fn login(config: &OAuthConfig) -> Result<Credentials, LoginError> {
    let verifier = generate_code_verifier();
    let challenge = code_challenge(&verifier);
    let state = generate_state();

    let url = authorization_url(config, &challenge, &state)?;
    eprintln!("Opening browser for Jira authorization:\n{url}\n");
    let _ = webbrowser::open(&url);

    let params = wait_for_callback(&state)?;

    let token = exchange_code_for_token(config, &params.code, &verifier)?;
    let cloud_id = fetch_cloud_id(&token.access_token)?;

    Ok(Credentials {
        access_token: token.access_token,
        refresh_token: token.refresh_token,
        expires_at: now_unix() + token.expires_in,
        cloud_id,
    })
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
        "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
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
    let body = serde_json::json!({
        "grant_type": "authorization_code",
        "client_id": config.client_id,
        "client_secret": config.client_secret,
        "code": code,
        "redirect_uri": config.redirect_uri,
        "code_verifier": code_verifier,
    });

    request_token(&body)
}

/// Exchanges a refresh token for a new access/refresh token pair.
/// Atlassian refresh tokens rotate on every use — the returned credentials must replace the stored ones.
///
/// Requires `credentials.refresh_token` to be `Some`. Callers must check this first
/// (`load_credentials` does); credentials with no refresh token (service accounts)
/// must be renewed via `login_client_credentials` instead.
pub fn refresh(config: &OAuthConfig, credentials: &Credentials) -> Result<Credentials, LoginError> {
    let refresh_token = credentials.refresh_token.as_ref().ok_or_else(|| {
        LoginError::Internal(
            "refresh() called on credentials with no refresh_token (service account \
             credentials cannot be refreshed this way — re-run `jira auth login`)"
                .to_string(),
        )
    })?;

    let body = serde_json::json!({
        "grant_type": "refresh_token",
        "client_id": config.client_id,
        "client_secret": config.client_secret,
        "refresh_token": refresh_token,
    });

    let token = request_token(&body)?;

    Ok(Credentials {
        access_token: token.access_token,
        refresh_token: token.refresh_token,
        expires_at: now_unix() + token.expires_in,
        cloud_id: credentials.cloud_id.clone(),
    })
}

/// Runs the OAuth 2.0 `client_credentials` flow for a service account: exchanges
/// the app's `client_id`/`client_secret` directly for an access token (no browser,
/// no user interaction), then resolves the Jira cloud id. The returned credentials
/// have no `refresh_token` — `load_credentials` renews an expired token by
/// re-running this flow.
pub fn login_client_credentials(config: &OAuthConfig) -> Result<Credentials, LoginError> {
    let body = serde_json::json!({
        "grant_type": "client_credentials",
        "client_id": config.client_id,
        "client_secret": config.client_secret,
        "audience": "api.atlassian.com",
    });

    let token = request_token(&body)?;
    let cloud_id = fetch_cloud_id(&token.access_token)?;

    Ok(Credentials {
        access_token: token.access_token,
        refresh_token: token.refresh_token,
        expires_at: now_unix() + token.expires_in,
        cloud_id,
    })
}

fn request_token(body: &serde_json::Value) -> Result<TokenResponse, LoginError> {
    let response = reqwest::blocking::Client::new()
        .post("https://auth.atlassian.com/oauth/token")
        .json(body)
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

fn fetch_cloud_id(access_token: &str) -> Result<String, LoginError> {
    let resources: Vec<AccessibleResource> = reqwest::blocking::Client::new()
        .get("https://api.atlassian.com/oauth/token/accessible-resources")
        .bearer_auth(access_token)
        .header("Accept", "application/json")
        .send()
        .map_err(|e| LoginError::TokenExchange(e.to_string()))?
        .json()
        .map_err(|e| LoginError::TokenExchange(e.to_string()))?;

    resources
        .into_iter()
        .next()
        .map(|r| r.id)
        .ok_or(LoginError::NoAccessibleResources)
}

/// Renews credentials whose access token has expired (or is about to).
/// Credentials with a `refresh_token` (3LO) are renewed via `refresh`; credentials
/// with none (service accounts) are renewed by re-running `login_client_credentials`.
///
/// Every caller that may encounter an expired access token (`load_credentials`,
/// `doctor`'s credentials check, ...) must go through this function rather than
/// calling `refresh` directly — `refresh` errors out for service account
/// credentials, which have no `refresh_token`.
pub fn renew(config: &OAuthConfig, credentials: &Credentials) -> Result<Credentials, LoginError> {
    match &credentials.refresh_token {
        Some(_) => refresh(config, credentials),
        None => login_client_credentials(config),
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
    let json = serde_json::to_string_pretty(credentials).map_err(|e| {
        LoginError::Internal(format!("failed to serialize credentials: {e}"))
    })?;
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
    /// `None` for service account (`client_credentials`) logins, which are
    /// renewed by re-running `login_client_credentials`.
    #[serde(default)]
    pub refresh_token: Option<String>,
    /// Unix timestamp (seconds) after which the access token is no longer valid.
    pub expires_at: u64,
    /// Jira Cloud instance ID, resolved once at login via the accessible-resources endpoint.
    pub cloud_id: String,
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

/// Builds the Atlassian authorization URL the user must open in a browser.
pub fn authorization_url(
    config: &OAuthConfig,
    code_challenge: &str,
    state: &str,
) -> Result<String, LoginError> {
    let params = [
        ("audience", "api.atlassian.com"),
        ("client_id", &config.client_id),
        ("scope", OAuthConfig::SCOPES),
        ("redirect_uri", &config.redirect_uri),
        ("state", state),
        ("response_type", "code"),
        ("prompt", "consent"),
        ("code_challenge", code_challenge),
        ("code_challenge_method", "S256"),
    ];

    let query = serde_urlencoded::to_string(params)
        .map_err(|e| LoginError::Internal(format!("failed to encode authorization URL: {e}")))?;
    Ok(format!("https://auth.atlassian.com/authorize?{query}"))
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

/// Path to the local credentials file: `<config_dir>/jira-cli/credentials.json`.
pub fn credentials_path(config_dir: &Path) -> PathBuf {
    config_dir.join("jira-cli").join("credentials.json")
}

#[cfg(test)]
#[path = "auth_tests.rs"]
mod tests;
