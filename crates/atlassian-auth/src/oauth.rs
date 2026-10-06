//! OAuth 2.0 authentication infrastructure shared by every Atlassian Cloud
//! product CLI in this workspace (Jira, Confluence, ...), supporting two
//! grant types:
//!
//! - **3LO + PKCE** (`login`) — interactive consent flow for a human Atlassian
//!   account: PKCE challenge generation, browser launch, local listener for the
//!   callback (PKCE and listener from `crates/oauth-user-login`), authorization code exchange, and cloud ID
//!   resolution via the accessible-resources endpoint. Issues a `refresh_token`.
//! - **`client_credentials`** (`login_client_credentials`) — non-interactive flow
//!   for a service account: exchanges `client_id`/`client_secret` directly for
//!   an access token, no browser involved. Issues no `refresh_token`.
//!
//! Other layers:
//!
//! - **Identities** (`Identity`) — every crate stores two identities side by
//!   side (issue #164): the service identity (`client_credentials`, the
//!   default) and the human (`--user`). Each has its own `app.json` section and
//!   its own credentials file.
//! - **App identity** (`AppConfig`, one `OAuthConfig` per section) — the static
//!   Atlassian OAuth app credentials loaded from a crate's `app.json`, with
//!   helpers for loading, validating and rewriting the file.
//! - **Session credentials** (`Credentials`) — the dynamic token set (access token,
//!   optional refresh token, expiry, cloud ID, optional site URL) persisted to
//!   the identity's `credentials-service.json` / `credentials-user.json`.
//!
//! `refresh` exchanges a refresh token for a new token pair. Atlassian refresh
//! tokens **rotate on every use** — the new pair must always be persisted immediately
//! to avoid invalidating the stored token. Credentials with no `refresh_token`
//! (service accounts) are renewed by re-running `login_client_credentials` instead.
//!
//! OAuth scopes are requested per product (Jira and Confluence grant different
//! scope strings) — every function that needs them takes `scopes: &str` rather
//! than hardcoding a product's scope list. Similarly, `app_config_path`/
//! `Identity::credentials_path` take a `cli_dir` (e.g. `"jira-cli"`, `"confluence-cli"`)
//! so each crate's config lives under its own directory.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

use crate::endpoints;

pub use oauth_user_login::Identity;

/// One OAuth 2.0 app: a section of `app.json` (see [`AppConfig`]).
/// Written by a crate's `init` command (or by hand); never modified by the CLI at runtime.
#[derive(Debug, PartialEq, Eq)]
pub struct OAuthConfig {
    pub client_id: String,
    pub client_secret: String,
    pub redirect_uri: String,
}

impl OAuthConfig {
    pub const REDIRECT_URI: &'static str = "http://localhost:8080/callback";
}

#[derive(Debug, Deserialize, Serialize)]
struct AppCredentials {
    client_id: String,
    client_secret: String,
}

/// A crate's `app.json`: one optional OAuth app per identity.
///
/// ```json
/// { "service": { "client_id": "...", "client_secret": "..." },
///   "user":    { "client_id": "...", "client_secret": "..." } }
/// ```
///
/// For Atlassian the two are different apps: `service` is a Service Account
/// credential, `user` a 3LO app. The pre-#164 flat shape (`client_id` at top
/// level) is rejected with [`OAuthConfigError::LegacyFormat`].
#[derive(Debug, PartialEq, Eq)]
pub struct AppConfig {
    pub service: Option<OAuthConfig>,
    pub user: Option<OAuthConfig>,
}

#[derive(Debug, Deserialize, Serialize)]
struct AppConfigFile {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    service: Option<AppCredentials>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    user: Option<AppCredentials>,
    /// Only read to recognise the legacy flat shape.
    #[serde(default, skip_serializing)]
    client_id: Option<serde_json::Value>,
}

impl AppConfig {
    /// Parses the contents of `app.json`.
    pub fn from_json(json: &str) -> Result<Self, OAuthConfigError> {
        let file: AppConfigFile =
            serde_json::from_str(json).map_err(|e| OAuthConfigError::InvalidJson(e.to_string()))?;
        if file.client_id.is_some() {
            return Err(OAuthConfigError::LegacyFormat);
        }
        let to_config = |app: AppCredentials| OAuthConfig {
            client_id: app.client_id,
            client_secret: app.client_secret,
            redirect_uri: OAuthConfig::REDIRECT_URI.to_string(),
        };
        Ok(AppConfig {
            service: file.service.map(to_config),
            user: file.user.map(to_config),
        })
    }

    /// Loads `app.json` at `path`.
    pub fn load(path: &Path) -> Result<Self, OAuthConfigError> {
        let raw = std::fs::read_to_string(path)
            .map_err(|_| OAuthConfigError::NotFound(path.to_path_buf()))?;
        Self::from_json(&raw)
    }

    /// Serialises back to the `app.json` shape (absent sections omitted,
    /// `redirect_uri` never stored).
    pub fn to_json(&self) -> Result<String, OAuthConfigError> {
        let to_file = |config: &OAuthConfig| AppCredentials {
            client_id: config.client_id.clone(),
            client_secret: config.client_secret.clone(),
        };
        let file = AppConfigFile {
            service: self.service.as_ref().map(to_file),
            user: self.user.as_ref().map(to_file),
            client_id: None,
        };
        serde_json::to_string_pretty(&file).map_err(|e| OAuthConfigError::InvalidJson(e.to_string()))
    }

    /// The OAuth app configured for `identity`, if its section is present.
    pub fn section(&self, identity: Identity) -> Option<&OAuthConfig> {
        match identity {
            Identity::Service => self.service.as_ref(),
            Identity::User => self.user.as_ref(),
        }
    }
}

/// Path to a crate's app credentials file: `<config_dir>/<cli_dir>/app.json`.
pub fn app_config_path(config_dir: &Path, cli_dir: &str) -> PathBuf {
    config_dir.join(cli_dir).join("app.json")
}

/// The single credentials file used before issue #164:
/// `<config_dir>/<cli_dir>/credentials.json`. No longer read; crates only
/// report it (`doctor`) so it can be deleted. See [`Identity::credentials_path`].
pub fn legacy_credentials_path(config_dir: &Path, cli_dir: &str) -> PathBuf {
    config_dir.join(cli_dir).join("credentials.json")
}

/// Path to the state kept between the two steps of a remote login:
/// `<config_dir>/<cli_dir>/pending-login.json`.
pub fn pending_login_path(config_dir: &Path, cli_dir: &str) -> PathBuf {
    config_dir.join(cli_dir).join("pending-login.json")
}

#[derive(Debug, PartialEq, Eq)]
pub enum OAuthConfigError {
    NotFound(PathBuf),
    InvalidJson(String),
    /// `app.json` still has the pre-#164 flat `client_id`/`client_secret`
    /// shape instead of `service`/`user` sections.
    LegacyFormat,
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
            OAuthConfigError::LegacyFormat => write!(
                f,
                "app credentials file uses the old single-identity format (client_id at top level) \
                 instead of \"service\"/\"user\" sections"
            ),
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum LoginError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("{0}")]
    CallbackListener(oauth_user_login::ListenerError),
    #[error("{0}")]
    Callback(oauth_user_login::WaitError),
    #[error("{0}")]
    PendingLogin(oauth_user_login::PendingLoginError),
    #[error("token exchange failed: {0}")]
    TokenExchange(String),
    #[error("no accessible Atlassian resources found for this account")]
    NoAccessibleResources,
    /// The accessible-resources call itself failed (network, status, JSON),
    /// e.g. after a successful code exchange — not a refused code.
    #[error("could not list the Atlassian sites this account can access ({0}). Retry the login")]
    AccessibleResources(String),
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
    #[serde(default)]
    scopes: Vec<String>,
    #[serde(default)]
    url: Option<String>,
}

/// Runs the full interactive OAuth 2.0 (3LO) + PKCE login flow:
/// opens the browser, waits for the local callback, exchanges the code for tokens,
/// resolves the Atlassian cloud id, and returns the resulting credentials.
///
/// `scopes` is a space-separated OAuth scope string, product-specific
/// (e.g. Jira's `"read:jira-work ..."` vs Confluence's `"read:confluence-content.all ..."`).
pub fn login(config: &OAuthConfig, scopes: &str) -> Result<Credentials, LoginError> {
    let verifier = oauth_user_login::generate_code_verifier();
    let challenge = oauth_user_login::code_challenge(&verifier);
    let state = oauth_user_login::generate_state();

    let url = authorization_url(config, &challenge, &state, scopes)?;
    // Bind before opening the browser, so a busy port fails before the person
    // has consented.
    let listener = oauth_user_login::bind_listener(endpoints::CALLBACK_LISTEN_ADDR)
        .map_err(LoginError::CallbackListener)?;
    eprintln!("Opening browser for Atlassian authorization:\n{url}\n");
    let _ = webbrowser::open(&url);

    let params = oauth_user_login::wait_for_callback(&listener, endpoints::CALLBACK_PATH, &state)
        .map_err(LoginError::Callback)?;

    let token = request_token(&authorization_code_body(config, &params.code, &verifier, &config.redirect_uri))?;
    credentials_from_code_exchange(token)
}

/// Step 1 of the two-step (`--remote`) login: saves a pending login at `path`
/// and returns the authorize URL to hand to the person. No browser, no port.
pub fn start_remote_login(
    config: &OAuthConfig,
    scopes: &str,
    redirect_uri: &str,
    path: &Path,
    now: u64,
) -> Result<(String, oauth_user_login::PendingLogin), LoginError> {
    let pending = oauth_user_login::PendingLogin::new(Some(redirect_uri), true, now);
    let challenge = pending
        .code_challenge()
        .ok_or_else(|| LoginError::Internal("a PKCE pending login has no verifier".to_string()))?;
    let remote_config = OAuthConfig {
        client_id: config.client_id.clone(),
        client_secret: config.client_secret.clone(),
        redirect_uri: redirect_uri.to_string(),
    };
    let url = authorization_url(&remote_config, &challenge, &pending.state, scopes)?;
    pending.save(path).map_err(LoginError::PendingLogin)?;
    Ok((url, pending))
}

/// Step 2: takes the pending login for `state` (single-use), exchanges `code`
/// with its stored verifier and redirect URI, and resolves the cloud id.
pub fn complete_remote_login(
    config: &OAuthConfig,
    path: &Path,
    code: &str,
    state: &str,
    now: u64,
) -> Result<Credentials, LoginError> {
    complete_remote_login_at(
        config,
        path,
        code,
        state,
        now,
        endpoints::ATLASSIAN_TOKEN_URL,
        endpoints::ATLASSIAN_ACCESSIBLE_RESOURCES_URL,
    )
}

/// [`complete_remote_login`] against explicit endpoints, so tests can point
/// it at a local server.
pub(crate) fn complete_remote_login_at(
    config: &OAuthConfig,
    path: &Path,
    code: &str,
    state: &str,
    now: u64,
    token_url: &str,
    resources_url: &str,
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
    let token = request_token_at(token_url, &authorization_code_body(config, code, verifier, redirect_uri))?;
    let resource = first_resource(fetch_accessible_resources_at(resources_url, &token.access_token)?)?;
    Ok(credentials_from(token, resource))
}

fn credentials_from_code_exchange(token: TokenResponse) -> Result<Credentials, LoginError> {
    let resource = fetch_primary_resource(&token.access_token)?;
    Ok(credentials_from(token, resource))
}

fn credentials_from(token: TokenResponse, resource: AccessibleResource) -> Credentials {
    Credentials {
        access_token: token.access_token,
        refresh_token: token.refresh_token,
        expires_at: now_unix() + token.expires_in,
        cloud_id: resource.id,
        site_url: resource.url,
    }
}

/// Current Unix time in seconds (the `now` the remote-login functions take).
pub fn now_unix() -> u64 {
    // Fallback to 0 if the system clock predates the Unix epoch (should never happen
    // on a real machine, but avoids a panic — a 0 timestamp causes the token to be
    // treated as expired and refreshed on the next call, which is safe behavior).
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

/// Body of the authorization-code exchange. `redirect_uri` must be the one
/// the authorize request used.
pub(crate) fn authorization_code_body(
    config: &OAuthConfig,
    code: &str,
    code_verifier: &str,
    redirect_uri: &str,
) -> serde_json::Value {
    serde_json::json!({
        "grant_type": "authorization_code",
        "client_id": config.client_id,
        "client_secret": config.client_secret,
        "code": code,
        "redirect_uri": redirect_uri,
        "code_verifier": code_verifier,
    })
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
             credentials cannot be refreshed this way — re-run the client_credentials \
             login flow instead)"
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
        site_url: credentials.site_url.clone(),
    })
}

/// Runs the OAuth 2.0 `client_credentials` flow for a service account: exchanges
/// the app's `client_id`/`client_secret` directly for an access token (no browser,
/// no user interaction), then resolves the Atlassian cloud id. The returned
/// credentials have no `refresh_token` — `load_credentials` renews an expired
/// token by re-running this flow.
pub fn login_client_credentials(config: &OAuthConfig) -> Result<Credentials, LoginError> {
    let body = serde_json::json!({
        "grant_type": "client_credentials",
        "client_id": config.client_id,
        "client_secret": config.client_secret,
        "audience": endpoints::ATLASSIAN_AUDIENCE,
    });

    let token = request_token(&body)?;
    let resource = fetch_primary_resource(&token.access_token)?;

    Ok(Credentials {
        access_token: token.access_token,
        refresh_token: token.refresh_token,
        expires_at: now_unix() + token.expires_in,
        cloud_id: resource.id,
        site_url: resource.url,
    })
}

fn request_token(body: &serde_json::Value) -> Result<TokenResponse, LoginError> {
    request_token_at(endpoints::ATLASSIAN_TOKEN_URL, body)
}

fn request_token_at(url: &str, body: &serde_json::Value) -> Result<TokenResponse, LoginError> {
    let response = reqwest::blocking::Client::new()
        .post(url)
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

fn fetch_accessible_resources(access_token: &str) -> Result<Vec<AccessibleResource>, LoginError> {
    fetch_accessible_resources_at(endpoints::ATLASSIAN_ACCESSIBLE_RESOURCES_URL, access_token)
}

fn fetch_accessible_resources_at(url: &str, access_token: &str) -> Result<Vec<AccessibleResource>, LoginError> {
    let failed = |e: reqwest::Error| LoginError::AccessibleResources(e.to_string());
    let response = reqwest::blocking::Client::new()
        .get(url)
        .bearer_auth(access_token)
        .header("Accept", "application/json")
        .send()
        .map_err(failed)?;
    let status = response.status();
    if !status.is_success() {
        let text = response.text().unwrap_or_default();
        return Err(LoginError::AccessibleResources(format!("{status}: {text}")));
    }
    response.json().map_err(failed)
}

fn first_resource(resources: Vec<AccessibleResource>) -> Result<AccessibleResource, LoginError> {
    resources.into_iter().next().ok_or(LoginError::NoAccessibleResources)
}

fn fetch_primary_resource(access_token: &str) -> Result<AccessibleResource, LoginError> {
    first_resource(fetch_accessible_resources(access_token)?)
}

/// Fetches the OAuth scopes granted to `access_token` for the resource matching
/// `cloud_id`, via the accessible-resources endpoint. Used by `doctor`'s
/// `oauth_scopes` check, distinct from product-side permission checks.
pub fn get_granted_scopes(access_token: &str, cloud_id: &str) -> Result<Vec<String>, LoginError> {
    let resources = fetch_accessible_resources(access_token)?;
    merge_scopes_for_cloud_id(&resources, cloud_id).ok_or(LoginError::NoAccessibleResources)
}

/// Merges the `scopes` of every entry in `resources` whose `id` matches
/// `cloud_id`. Atlassian's accessible-resources endpoint returns **multiple
/// entries with the same `id`** when a single token's scopes span more than
/// one product (e.g. a Service Account credential granted both Jira and
/// Confluence scopes) — each entry holds one product's scope subset, not
/// their union — so taking only the first matching entry silently
/// under-reports the granted scopes. Returns `None` only if no entry matches
/// `cloud_id` at all; an entry that matches but grants zero scopes still
/// returns `Some(vec![])`, since those are different situations (no access
/// to this site at all vs. access with nothing granted).
fn merge_scopes_for_cloud_id(resources: &[AccessibleResource], cloud_id: &str) -> Option<Vec<String>> {
    let mut matched = false;
    let mut scopes = Vec::new();
    for resource in resources {
        if resource.id == cloud_id {
            matched = true;
            for scope in &resource.scopes {
                if !scopes.contains(scope) {
                    scopes.push(scope.clone());
                }
            }
        }
    }
    matched.then_some(scopes)
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
    /// Atlassian Cloud site ID, resolved once at login via the accessible-resources endpoint.
    pub cloud_id: String,
    /// Atlassian site's browsable base URL (e.g. "<https://mysite.atlassian.net>"),
    /// resolved once at login via the accessible-resources endpoint's `url` field.
    /// `None` for credentials saved before this field existed — re-run `auth login`
    /// to populate it. Carried forward unchanged by `refresh()`, never re-fetched,
    /// same as `cloud_id`.
    #[serde(default)]
    pub site_url: Option<String>,
}

/// Builds the Atlassian authorization URL the user must open in a browser.
pub fn authorization_url(
    config: &OAuthConfig,
    code_challenge: &str,
    state: &str,
    scopes: &str,
) -> Result<String, LoginError> {
    let params = [
        ("audience", endpoints::ATLASSIAN_AUDIENCE),
        ("client_id", &config.client_id),
        ("scope", scopes),
        ("redirect_uri", &config.redirect_uri),
        ("state", state),
        ("response_type", "code"),
        ("prompt", "consent"),
        ("code_challenge", code_challenge),
        ("code_challenge_method", "S256"),
    ];

    let query = serde_urlencoded::to_string(params)
        .map_err(|e| LoginError::Internal(format!("failed to encode authorization URL: {e}")))?;
    Ok(format!("{}?{query}", endpoints::ATLASSIAN_AUTHORIZE_URL))
}

#[cfg(test)]
#[path = "tests/oauth_tests.rs"]
mod tests;
