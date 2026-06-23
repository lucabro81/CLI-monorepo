//! Google Chat API v1 HTTP client.
//!
//! `GoogleChatClient` wraps a blocking `reqwest` client pre-configured with a
//! `Bearer` token and the base URL `https://chat.googleapis.com/v1`. All
//! methods return raw `serde_json::Value` so callers decide how much
//! structure to impose; the `--select` flag can then filter the output
//! client-side without requiring typed response structs for every endpoint.
//!
//! Unlike jira/bitbucket, there is no tenant ID to splice into the base URL
//! (no Jira-style `cloud_id`) — every call is scoped by the access token's
//! identity alone.

use crate::auth::Credentials;
use crate::endpoints;

/// Error returned by `GoogleChatClient` methods.
///
/// `Request` covers network-level failures (connection refused, timeout, etc.).
/// `Status` covers HTTP-level failures where the server responded with a non-2xx status.
#[derive(Debug)]
pub enum ClientError {
    /// Network or serialization error — no HTTP response was received.
    Request(String),
    /// The server responded but with a non-2xx status code.
    Status { status: u16, body: String },
}

impl std::fmt::Display for ClientError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ClientError::Request(msg) => write!(f, "request failed: {msg}"),
            ClientError::Status { status, body } => {
                write!(f, "Google Chat API returned status {status}: {body}")
            }
        }
    }
}

/// Blocking HTTP client for the Google Chat API v1.
pub struct GoogleChatClient {
    access_token: String,
    http: reqwest::blocking::Client,
}

impl GoogleChatClient {
    /// Builds a client from stored credentials.
    pub fn new(credentials: &Credentials) -> Self {
        Self {
            access_token: credentials.access_token.clone(),
            http: reqwest::blocking::Client::new(),
        }
    }

    /// Lists spaces the authenticated identity belongs to, as raw JSON
    /// (`{"spaces": [...], "nextPageToken": "..."}`).
    pub fn list_spaces(
        &self,
        page_size: u32,
        page_token: Option<&str>,
    ) -> Result<serde_json::Value, ClientError> {
        let mut pairs: Vec<(&str, String)> = vec![("pageSize", page_size.to_string())];
        if let Some(token) = page_token {
            pairs.push(("pageToken", token.to_string()));
        }
        let params = serde_urlencoded::to_string(&pairs)
            .map_err(|e| ClientError::Request(format!("failed to encode query params: {e}")))?;

        self.get_json(&format!("{}?{params}", endpoints::PATH_SPACES))
    }

    fn get_json(&self, path: &str) -> Result<serde_json::Value, ClientError> {
        self.get_json_absolute(&format!("{}{path}", endpoints::CHAT_API_BASE_URL))
    }

    fn get_json_absolute(&self, url: &str) -> Result<serde_json::Value, ClientError> {
        let response = self
            .http
            .get(url)
            .bearer_auth(&self.access_token)
            .header("Accept", "application/json")
            .send()
            .map_err(|e| ClientError::Request(e.to_string()))?;

        let status = response.status();
        if !status.is_success() {
            let body = response.text().unwrap_or_default();
            return Err(ClientError::Status {
                status: status.as_u16(),
                body,
            });
        }

        response
            .json::<serde_json::Value>()
            .map_err(|e| ClientError::Request(e.to_string()))
    }
}
