//! Atlassian Organization Admin API HTTP client.
//!
//! `AdminClient` wraps a blocking `reqwest` client pre-configured with the
//! Organization API key as a Bearer token and the base URL
//! `https://api.atlassian.com/admin`. Methods return raw `serde_json::Value`
//! so callers decide how much structure to impose; the `--select` flag can
//! then filter the output client-side.

use crate::auth::AdminConfig;
use crate::endpoints;

/// Error returned by `AdminClient` methods.
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
                write!(f, "Atlassian Admin API returned status {status}: {body}")
            }
        }
    }
}

/// Blocking HTTP client for the Atlassian Organization Admin API.
pub struct AdminClient {
    base_url: String,
    org_id: String,
    api_key: String,
    http: reqwest::blocking::Client,
}

impl AdminClient {
    /// Builds a client from the static app config (`app.json`).
    pub fn new(config: &AdminConfig) -> Self {
        Self {
            base_url: endpoints::ADMIN_API_BASE_URL.to_string(),
            org_id: config.org_id.clone(),
            api_key: config.api_key.clone(),
            http: reqwest::blocking::Client::new(),
        }
    }

    /// Returns the organization's own info, as raw JSON. Used by `doctor` as a
    /// lightweight live check that the API key and org id work together.
    pub fn get_organization(&self) -> Result<serde_json::Value, ClientError> {
        self.get_json(&endpoints::path_organization(&self.org_id))
    }

    /// Resolves `account_id` (an Atlassian identity shared across Jira,
    /// Confluence, and Bitbucket) to a managed-account profile, as raw JSON.
    /// Only resolves accounts whose email domain is managed under this
    /// organization; other accounts return a non-2xx status.
    pub fn get_user(&self, account_id: &str) -> Result<serde_json::Value, ClientError> {
        self.get_json(&endpoints::path_user_manage(&self.org_id, account_id))
    }

    fn get_json(&self, path: &str) -> Result<serde_json::Value, ClientError> {
        let url = format!("{}{path}", self.base_url);

        let response = self
            .http
            .get(&url)
            .bearer_auth(&self.api_key)
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
            .json()
            .map_err(|e| ClientError::Request(e.to_string()))
    }
}
