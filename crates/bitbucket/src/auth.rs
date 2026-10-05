//! OAuth 2.0 authentication for a Bitbucket OAuth consumer, with two grants:
//!
//! - **`client_credentials`** (`auth login`, default) — the consumer's
//!   `client_id`/`client_secret` are exchanged directly for an access token via
//!   HTTP Basic auth, no browser or user interaction. Every action is attributed
//!   to the OAuth app. No `refresh_token` — renewed by re-running the exchange.
//! - **`authorization_code`** (`auth login --user`) — interactive: the browser
//!   opens Bitbucket's consent page, Bitbucket redirects to the consumer's
//!   callback URL (`http://localhost:8080/callback`), and the code is exchanged
//!   for tokens. Every action is attributed to the human who consented. Issues
//!   a `refresh_token`, which rotates on every use. Bitbucket documents no PKCE
//!   and no `redirect_uri` parameter for this grant; `state` guards against CSRF.
//!   `state` and the callback listener come from `crates/oauth-user-login`.
//!
//! - **App identity** (`OAuthConfig`) — the static OAuth consumer credentials
//!   loaded from `app.json`.
//! - **Session credentials** (`Credentials`) — the access token, its expiry and,
//!   for `--user` logins, the refresh token, persisted to `credentials.json`.

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

/// Path to the pending remote login: `<config_dir>/bitbucket-cli/pending-login.json`.
pub fn pending_login_path(config_dir: &Path) -> PathBuf {
    config_dir.join("bitbucket-cli").join("pending-login.json")
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
    #[error("{0}: bitbucket auth login --user")]
    CallbackListener(oauth_user_login::ListenerError),
    #[error("{0}: bitbucket auth login --user")]
    Callback(oauth_user_login::WaitError),
    #[error("{0}")]
    PendingLogin(oauth_user_login::PendingLoginError),
    /// A condition that should be unreachable given valid inputs.
    /// If this surfaces it indicates a bug in the CLI itself.
    #[error("internal error: {0}")]
    Internal(String),
}

#[derive(Debug, Deserialize)]
struct TokenResponse {
    access_token: String,
    expires_in: u64,
    /// Space-separated list of OAuth scopes granted, e.g.
    /// "repository:read pullrequest:write account:read". The observed
    /// `client_credentials` response uses the standard `OAuth2` (RFC 6749) field
    /// name "scope" (singular); Bitbucket's documented `authorization_code` /
    /// `refresh_token` responses use "scopes". Both are accepted; absent means
    /// no scopes reported (surfaced by `doctor`, not a login failure).
    #[serde(default, alias = "scopes")]
    scope: String,
    /// Present for the `authorization_code` and `refresh_token` grants; absent
    /// for `client_credentials`.
    #[serde(default)]
    refresh_token: Option<String>,
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
    /// Set only for `auth login --user` (human identity); `None` means
    /// `client_credentials` (app identity). Decides how the token is renewed.
    /// Defaulted so credentials files written before `--user` existed still load.
    #[serde(default)]
    pub refresh_token: Option<String>,
}

pub(crate) fn now_unix() -> u64 {
    // Fallback to 0 if the system clock predates the Unix epoch (should never happen
    // on a real machine, but avoids a panic — a 0 timestamp causes the token to be
    // treated as expired and renewed on the next call, which is safe behavior).
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

/// Parses a token endpoint's response body into a `TokenResponse`. On failure
/// the error names the fields that arrived, so an unexpected response shape
/// (e.g. an unfamiliar field name) is self-diagnosing — but never their values,
/// which may be tokens.
fn parse_token_response(text: &str) -> Result<TokenResponse, LoginError> {
    serde_json::from_str(text).map_err(|e| {
        let shape = match serde_json::from_str::<serde_json::Value>(text) {
            Ok(serde_json::Value::Object(fields)) => {
                let mut names: Vec<&str> = fields.keys().map(String::as_str).collect();
                names.sort_unstable();
                format!("response fields: {}", names.join(", "))
            }
            Ok(_) => "response was JSON but not an object".to_string(),
            Err(_) => format!("response was not JSON ({} bytes)", text.len()),
        };
        LoginError::TokenExchange(format!("{e}; {shape}"))
    })
}

/// Builds `Credentials` from a token response received at `now`. If the
/// response carries no `refresh_token`, `previous_refresh_token` is kept, so a
/// `--user` session can never silently degrade into the app identity.
fn credentials_from_token(
    token: TokenResponse,
    now: u64,
    previous_refresh_token: Option<String>,
) -> Credentials {
    Credentials {
        access_token: token.access_token,
        expires_at: now + token.expires_in,
        scopes: token.scope.split_whitespace().map(str::to_string).collect(),
        refresh_token: token.refresh_token.or(previous_refresh_token),
    }
}

/// POSTs `form` to the token endpoint (HTTP Basic auth with the consumer's
/// credentials) and parses the response.
fn request_token(config: &OAuthConfig, form: &[(&str, String)]) -> Result<TokenResponse, LoginError> {
    request_token_at(endpoints::BITBUCKET_TOKEN_URL, config, form)
}

fn request_token_at(
    url: &str,
    config: &OAuthConfig,
    form: &[(&str, String)],
) -> Result<TokenResponse, LoginError> {
    let response = reqwest::blocking::Client::new()
        .post(url)
        .basic_auth(&config.client_id, Some(&config.client_secret))
        .form(form)
        .send()
        .map_err(|e| LoginError::TokenExchange(e.to_string()))?;

    let status = response.status();
    let text = response
        .text()
        .map_err(|e| LoginError::TokenExchange(e.to_string()))?;

    if !status.is_success() {
        return Err(LoginError::TokenExchange(format!("{status}: {text}")));
    }

    parse_token_response(&text)
}

/// Runs the OAuth 2.0 `client_credentials` flow: exchanges the OAuth consumer's
/// `client_id`/`client_secret` (via HTTP Basic auth) for an access token. No
/// browser, no user interaction, no refresh token.
pub fn login_client_credentials(config: &OAuthConfig) -> Result<Credentials, LoginError> {
    let token = request_token(config, &[("grant_type", "client_credentials".to_string())])?;
    Ok(credentials_from_token(token, now_unix(), None))
}

/// Runs the interactive OAuth 2.0 `authorization_code` flow: opens the browser
/// on Bitbucket's consent page, waits for the callback on
/// [`endpoints::CALLBACK_LISTEN_ADDRESS`], and exchanges the code for tokens.
pub fn login(config: &OAuthConfig) -> Result<Credentials, LoginError> {
    // Bind before opening the browser, so a busy port fails fast instead of
    // after the user has already consented.
    let listener = oauth_user_login::bind_listener(endpoints::CALLBACK_LISTEN_ADDRESS)
        .map_err(LoginError::CallbackListener)?;

    let state = oauth_user_login::generate_state();
    let url = authorization_url(config, &state)?;
    eprintln!("Opening the browser for Bitbucket consent. If it does not open, visit:\n{url}");
    let _ = webbrowser::open(&url);

    let params = oauth_user_login::wait_for_callback(&listener, endpoints::CALLBACK_PATH, &state)
        .map_err(LoginError::Callback)?;

    let token = request_token(config, &authorization_code_form(&params.code))?;
    Ok(credentials_from_token(token, now_unix(), None))
}

/// Step 1 of the two-step (`--remote`) login: saves a pending login (state
/// only: Bitbucket has no PKCE and no `redirect_uri`, it always redirects to
/// the consumer's callback URL) and returns the consent URL. No browser, no port.
pub fn start_remote_login(
    config: &OAuthConfig,
    path: &Path,
    now: u64,
) -> Result<(String, oauth_user_login::PendingLogin), LoginError> {
    let pending = oauth_user_login::PendingLogin::new(None, false, now);
    let url = authorization_url(config, &pending.state)?;
    pending.save(path).map_err(LoginError::PendingLogin)?;
    Ok((url, pending))
}

/// Step 2: takes the pending login for `state` (single-use) and exchanges `code`.
pub fn complete_remote_login(
    config: &OAuthConfig,
    path: &Path,
    code: &str,
    state: &str,
    now: u64,
) -> Result<Credentials, LoginError> {
    complete_remote_login_at(config, path, code, state, now, endpoints::BITBUCKET_TOKEN_URL)
}

/// [`complete_remote_login`] against an explicit token endpoint, so tests can
/// point it at a local server.
pub(crate) fn complete_remote_login_at(
    config: &OAuthConfig,
    path: &Path,
    code: &str,
    state: &str,
    now: u64,
    token_url: &str,
) -> Result<Credentials, LoginError> {
    oauth_user_login::take_pending_login(path, state, now).map_err(LoginError::PendingLogin)?;
    let token = request_token_at(token_url, config, &authorization_code_form(code))?;
    Ok(credentials_from_token(token, now, None))
}

/// Builds Bitbucket's authorization URL the user must open in a browser.
pub fn authorization_url(config: &OAuthConfig, state: &str) -> Result<String, LoginError> {
    let params = [
        ("client_id", config.client_id.as_str()),
        ("response_type", "code"),
        ("state", state),
    ];

    let query = serde_urlencoded::to_string(params)
        .map_err(|e| LoginError::Internal(format!("failed to encode authorization URL: {e}")))?;
    Ok(format!("{}?{query}", endpoints::BITBUCKET_AUTHORIZE_URL))
}

/// Form body exchanging an authorization code for tokens.
fn authorization_code_form(code: &str) -> Vec<(&'static str, String)> {
    vec![
        ("grant_type", "authorization_code".to_string()),
        ("code", code.to_string()),
    ]
}

/// Form body renewing `credentials`: the `refresh_token` grant for `--user`
/// credentials, `client_credentials` otherwise.
fn renewal_form(credentials: &Credentials) -> Vec<(&'static str, String)> {
    match &credentials.refresh_token {
        Some(refresh_token) => vec![
            ("grant_type", "refresh_token".to_string()),
            ("refresh_token", refresh_token.clone()),
        ],
        None => vec![("grant_type", "client_credentials".to_string())],
    }
}

/// Renews `credentials` through the grant matching the identity they were
/// issued for. Bitbucket rotates refresh tokens, so the result must replace
/// the stored credentials.
pub fn renew(config: &OAuthConfig, credentials: &Credentials) -> Result<Credentials, LoginError> {
    let token = request_token(config, &renewal_form(credentials))?;
    Ok(credentials_from_token(
        token,
        now_unix(),
        credentials.refresh_token.clone(),
    ))
}

/// Loads credentials from disk, renewing them first if the access token has
/// expired (or is about to, within 60s).
pub fn load_credentials(config: &OAuthConfig, path: &Path) -> Result<Credentials, LoginError> {
    let raw = std::fs::read_to_string(path)?;
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
