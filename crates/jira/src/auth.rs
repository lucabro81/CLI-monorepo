use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, PartialEq, Eq)]
pub struct OAuthConfig {
    pub client_id: String,
    pub client_secret: String,
    pub redirect_uri: String,
}

impl OAuthConfig {
    pub const SCOPES: &'static str = "read:jira-work read:jira-user offline_access";
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

#[derive(Debug)]
pub enum LoginError {
    Io(std::io::Error),
    Callback(CallbackError),
    StateMismatch,
    TokenExchange(String),
    NoAccessibleResources,
}

impl std::fmt::Display for LoginError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LoginError::Io(e) => write!(f, "I/O error: {e}"),
            LoginError::Callback(e) => write!(f, "invalid callback: {e:?}"),
            LoginError::StateMismatch => {
                write!(f, "OAuth state mismatch — possible CSRF, aborting")
            }
            LoginError::TokenExchange(msg) => write!(f, "token exchange failed: {msg}"),
            LoginError::NoAccessibleResources => {
                write!(f, "no accessible Jira sites returned for this account")
            }
        }
    }
}

#[derive(Debug, Deserialize)]
struct TokenResponse {
    access_token: String,
    refresh_token: String,
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

    let url = authorization_url(config, &challenge, &state);
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
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("system clock is before unix epoch")
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
pub fn refresh(config: &OAuthConfig, credentials: &Credentials) -> Result<Credentials, LoginError> {
    let body = serde_json::json!({
        "grant_type": "refresh_token",
        "client_id": config.client_id,
        "client_secret": config.client_secret,
        "refresh_token": credentials.refresh_token,
    });

    let token = request_token(&body)?;

    Ok(Credentials {
        access_token: token.access_token,
        refresh_token: token.refresh_token,
        expires_at: now_unix() + token.expires_in,
        cloud_id: credentials.cloud_id.clone(),
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

/// Loads credentials from disk, refreshing them first if the access token has expired.
pub fn load_credentials(config: &OAuthConfig, path: &Path) -> Result<Credentials, LoginError> {
    let raw = std::fs::read_to_string(path).map_err(LoginError::Io)?;
    let credentials: Credentials =
        serde_json::from_str(&raw).map_err(|e| LoginError::TokenExchange(e.to_string()))?;

    if now_unix() + 60 >= credentials.expires_at {
        let refreshed = refresh(config, &credentials)?;
        save_credentials(path, &refreshed)?;
        return Ok(refreshed);
    }

    Ok(credentials)
}

pub fn save_credentials(path: &Path, credentials: &Credentials) -> Result<(), LoginError> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(LoginError::Io)?;
    }
    let json = serde_json::to_string_pretty(credentials)
        .expect("credentials always serialize");
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Credentials {
    pub access_token: String,
    pub refresh_token: String,
    pub expires_at: u64,
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
pub fn authorization_url(config: &OAuthConfig, code_challenge: &str, state: &str) -> String {
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

    let query = serde_urlencoded::to_string(params).expect("query params always serialize");
    format!("https://auth.atlassian.com/authorize?{query}")
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
mod tests {
    use super::*;

    #[test]
    fn code_verifier_is_url_safe_and_long_enough() {
        let verifier = generate_code_verifier();

        assert!(verifier.len() >= 43 && verifier.len() <= 128);
        assert!(verifier
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_'));
    }

    #[test]
    fn code_verifiers_are_random() {
        assert_ne!(generate_code_verifier(), generate_code_verifier());
    }

    #[test]
    fn code_challenge_matches_known_rfc7636_example() {
        // From RFC 7636 appendix B.
        let verifier = "dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk";

        assert_eq!(code_challenge(verifier), "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM");
    }

    #[test]
    fn state_values_are_random() {
        assert_ne!(generate_state(), generate_state());
    }

    #[test]
    fn builds_authorization_url_with_required_params() {
        let config = OAuthConfig {
            client_id: "my-client-id".to_string(),
            client_secret: "shh".to_string(),
            redirect_uri: "http://localhost:8080/callback".to_string(),
        };

        let url = authorization_url(&config, "challenge123", "state456");

        assert!(url.starts_with("https://auth.atlassian.com/authorize?"));
        assert!(url.contains("client_id=my-client-id"));
        assert!(url.contains("code_challenge=challenge123"));
        assert!(url.contains("code_challenge_method=S256"));
        assert!(url.contains("state=state456"));
        assert!(url.contains("response_type=code"));
        assert!(url.contains("audience=api.atlassian.com"));
        assert!(url.contains("redirect_uri=http%3A%2F%2Flocalhost%3A8080%2Fcallback"));
        assert!(url.contains("scope=read%3Ajira-work+read%3Ajira-user+offline_access"));
    }

    #[test]
    fn parses_valid_callback_request_line() {
        let line = "GET /callback?code=abc123&state=xyz789 HTTP/1.1";

        let params = parse_callback_request_line(line).expect("should parse");

        assert_eq!(
            params,
            CallbackParams {
                code: "abc123".to_string(),
                state: "xyz789".to_string(),
            }
        );
    }

    #[test]
    fn rejects_callback_missing_code() {
        let line = "GET /callback?state=xyz789 HTTP/1.1";

        assert_eq!(
            parse_callback_request_line(line),
            Err(CallbackError::MissingParam("code"))
        );
    }

    #[test]
    fn rejects_callback_missing_state() {
        let line = "GET /callback?code=abc123 HTTP/1.1";

        assert_eq!(
            parse_callback_request_line(line),
            Err(CallbackError::MissingParam("state"))
        );
    }

    #[test]
    fn rejects_malformed_request_line() {
        assert_eq!(
            parse_callback_request_line("not a request line"),
            Err(CallbackError::MalformedRequestLine)
        );
    }

    #[test]
    fn credentials_round_trip_through_json() {
        let creds = Credentials {
            access_token: "access".to_string(),
            refresh_token: "refresh".to_string(),
            expires_at: 1_700_000_000,
            cloud_id: "cloud-123".to_string(),
        };

        let json = serde_json::to_string(&creds).unwrap();
        let parsed: Credentials = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed, creds);
    }

    #[test]
    fn credentials_path_is_under_jira_cli_dir() {
        let path = credentials_path(Path::new("/home/user/.config"));

        assert_eq!(
            path,
            PathBuf::from("/home/user/.config/jira-cli/credentials.json")
        );
    }

    #[test]
    fn app_config_path_is_under_jira_cli_dir() {
        let path = app_config_path(Path::new("/home/user/.config"));

        assert_eq!(
            path,
            PathBuf::from("/home/user/.config/jira-cli/app.json")
        );
    }

    #[test]
    fn parses_oauth_config_from_app_json() {
        let json = r#"{"client_id": "abc", "client_secret": "shh"}"#;

        let config = OAuthConfig::from_json(json).expect("should parse");

        assert_eq!(
            config,
            OAuthConfig {
                client_id: "abc".to_string(),
                client_secret: "shh".to_string(),
                redirect_uri: OAuthConfig::REDIRECT_URI.to_string(),
            }
        );
    }

    #[test]
    fn rejects_malformed_app_json() {
        let result = OAuthConfig::from_json("not json");

        assert!(matches!(result, Err(OAuthConfigError::InvalidJson(_))));
    }
}
