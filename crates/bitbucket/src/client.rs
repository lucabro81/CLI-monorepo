//! Bitbucket REST API v2.0 HTTP client.
//!
//! `BitbucketClient` wraps a blocking `reqwest` client pre-configured with a
//! `Bearer` token and the base URL `https://api.bitbucket.org/2.0`. All
//! methods return raw `serde_json::Value` so callers decide how much
//! structure to impose; the `--select` flag can then filter the output
//! client-side without requiring typed response structs for every endpoint.

use crate::auth::Credentials;
use crate::endpoints;

/// Error returned by `BitbucketClient` methods.
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
                write!(f, "Bitbucket returned status {status}: {body}")
            }
        }
    }
}

/// Blocking HTTP client for the Bitbucket REST API v2.0.
pub struct BitbucketClient {
    base_url: String,
    access_token: String,
    http: reqwest::blocking::Client,
}

impl BitbucketClient {
    /// Builds a client from stored credentials.
    pub fn new(credentials: &Credentials) -> Self {
        Self {
            base_url: endpoints::BITBUCKET_API_BASE_URL.to_string(),
            access_token: credentials.access_token.clone(),
            http: reqwest::blocking::Client::new(),
        }
    }

    /// Returns the account associated with the access token, as raw JSON.
    pub fn get_current_user(&self) -> Result<serde_json::Value, ClientError> {
        self.get_json(endpoints::PATH_USER)
    }

    /// Returns the repository identified by `workspace`/`repo_slug`, as raw JSON.
    pub fn get_repository(&self, workspace: &str, repo_slug: &str) -> Result<serde_json::Value, ClientError> {
        self.get_json(&endpoints::path_repository(workspace, repo_slug))
    }

    /// Returns a page of repositories in `workspace`, as raw JSON.
    pub fn list_repositories(&self, workspace: &str, page: Option<u32>) -> Result<serde_json::Value, ClientError> {
        self.get_json(&endpoints::path_repositories(workspace, page))
    }

    /// Returns a page of pull requests for `workspace`/`repo_slug`, as raw JSON.
    /// `state` filters to `OPEN`, `MERGED`, `DECLINED`, or `SUPERSEDED`; `None` returns
    /// pull requests in any state.
    pub fn list_pull_requests(
        &self,
        workspace: &str,
        repo_slug: &str,
        state: Option<&str>,
        page: Option<u32>,
    ) -> Result<serde_json::Value, ClientError> {
        self.get_json(&endpoints::path_pull_requests(workspace, repo_slug, state, page))
    }

    /// Creates a repository at `workspace`/`repo_slug` with the given JSON body.
    /// Returns the created repository, as raw JSON.
    pub fn create_repository(
        &self,
        workspace: &str,
        repo_slug: &str,
        body: &serde_json::Value,
    ) -> Result<serde_json::Value, ClientError> {
        self.post_json(&endpoints::path_repository(workspace, repo_slug), body)
    }

    fn get_json(&self, path: &str) -> Result<serde_json::Value, ClientError> {
        let url = format!("{}{path}", self.base_url);

        let response = self
            .http
            .get(&url)
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
            .json()
            .map_err(|e| ClientError::Request(e.to_string()))
    }

    fn post_json(&self, path: &str, body: &serde_json::Value) -> Result<serde_json::Value, ClientError> {
        let url = format!("{}{path}", self.base_url);

        let response = self
            .http
            .post(&url)
            .bearer_auth(&self.access_token)
            .header("Accept", "application/json")
            .json(body)
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
