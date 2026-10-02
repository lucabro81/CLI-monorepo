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
    #[error(
        "cannot listen for the login callback on {address}: {reason}. \
        Another process is probably using that port: stop it (e.g. find it with `lsof -i :8080`) and retry"
    )]
    CallbackListener { address: String, reason: String },
    #[error("{0}")]
    Callback(#[from] CallbackError),
    #[error(
        "the login callback's state did not match the one sent (possible CSRF or a stale browser tab). \
        Retry: bitbucket auth login --user"
    )]
    StateMismatch,
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

fn now_unix() -> u64 {
    // Fallback to 0 if the system clock predates the Unix epoch (should never happen
    // on a real machine, but avoids a panic — a 0 timestamp causes the token to be
    // treated as expired and renewed on the next call, which is safe behavior).
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

/// Parses a token endpoint's response body into a `TokenResponse`. On failure,
/// the raw body is included in the error so an unexpected response shape
/// (e.g. an unfamiliar field name) is self-diagnosing without a manual request.
fn parse_token_response(text: &str) -> Result<TokenResponse, LoginError> {
    serde_json::from_str(text).map_err(|e| LoginError::TokenExchange(format!("{e}: {text}")))
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
    let response = reqwest::blocking::Client::new()
        .post(endpoints::BITBUCKET_TOKEN_URL)
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
    let listener = std::net::TcpListener::bind(endpoints::CALLBACK_LISTEN_ADDRESS).map_err(|e| {
        LoginError::CallbackListener {
            address: endpoints::CALLBACK_LISTEN_ADDRESS.to_string(),
            reason: e.to_string(),
        }
    })?;

    let state = generate_state();
    let url = authorization_url(config, &state)?;
    eprintln!("Opening the browser for Bitbucket consent. If it does not open, visit:\n{url}");
    let _ = webbrowser::open(&url);

    let params = wait_for_callback(&listener)?;
    if params.state != state {
        return Err(LoginError::StateMismatch);
    }

    let token = request_token(config, &authorization_code_form(&params.code))?;
    Ok(credentials_from_token(token, now_unix(), None))
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

/// Generates a random opaque state string used to protect against CSRF in the OAuth flow.
pub fn generate_state() -> String {
    use base64::Engine;
    use rand::RngCore;

    let mut bytes = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut bytes);
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes)
}

/// Accepts connections on `listener` until one carries the authorization
/// callback, replies with a small confirmation page, and returns its params.
/// Requests without a query string (e.g. a browser's `/favicon.ico`) are skipped.
fn wait_for_callback(listener: &std::net::TcpListener) -> Result<CallbackParams, LoginError> {
    use std::io::{BufRead, BufReader, Write};

    loop {
        let (mut stream, _) = listener.accept()?;
        let mut request_line = String::new();
        BufReader::new(&stream).read_line(&mut request_line)?;

        let result = parse_callback_request_line(request_line.trim_end());
        if result == Err(CallbackError::MalformedRequestLine) {
            let _ = stream.write_all(b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n");
            continue;
        }

        let body = match &result {
            Ok(_) => "Login complete. You can close this window and return to the terminal.",
            Err(_) => "Login failed. Return to the terminal for details.",
        };
        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: text/plain; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        );
        let _ = stream.write_all(response.as_bytes());

        return Ok(result?);
    }
}

/// `code` and `state` extracted from the authorization callback.
#[derive(Debug, PartialEq, Eq)]
pub struct CallbackParams {
    pub code: String,
    pub state: String,
}

#[derive(Debug, PartialEq, Eq, thiserror::Error)]
pub enum CallbackError {
    #[error("the login callback request was malformed. Retry: bitbucket auth login --user")]
    MalformedRequestLine,
    #[error(
        "the login callback is missing the `{0}` parameter. \
        Check that the OAuth consumer's callback URL is exactly http://localhost:8080/callback, then retry: bitbucket auth login --user"
    )]
    MissingParam(&'static str),
    #[error(
        "Bitbucket refused the authorization: {error}{}. \
        Approve the consent page to continue, then retry: bitbucket auth login --user",
        description.as_deref().map(|d| format!(" ({d})")).unwrap_or_default()
    )]
    Denied {
        error: String,
        description: Option<String>,
    },
}

/// Parses the first line of the local callback HTTP request, e.g.
/// `GET /callback?code=XYZ&state=abc HTTP/1.1`, extracting `code` and `state`.
/// An `error` parameter (e.g. the user clicked "Deny") takes precedence.
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

    let mut pairs: std::collections::HashMap<String, String> =
        serde_urlencoded::from_str(query).map_err(|_| CallbackError::MalformedRequestLine)?;

    if let Some(error) = pairs.remove("error") {
        return Err(CallbackError::Denied {
            error,
            description: pairs.remove("error_description"),
        });
    }

    Ok(CallbackParams {
        code: pairs.remove("code").ok_or(CallbackError::MissingParam("code"))?,
        state: pairs.remove("state").ok_or(CallbackError::MissingParam("state"))?,
    })
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
