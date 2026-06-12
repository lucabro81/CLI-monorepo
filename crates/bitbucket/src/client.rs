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

    /// Returns a page of branches in `workspace`/`repo_slug`, as raw JSON.
    pub fn list_branches(&self, workspace: &str, repo_slug: &str, page: Option<u32>) -> Result<serde_json::Value, ClientError> {
        self.get_json(&endpoints::path_branches(workspace, repo_slug, page))
    }

    /// Returns the pull request identified by `id` in `workspace`/`repo_slug`, as raw JSON.
    pub fn get_pull_request(&self, workspace: &str, repo_slug: &str, id: u64) -> Result<serde_json::Value, ClientError> {
        self.get_json(&endpoints::path_pull_request(workspace, repo_slug, id))
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

    /// Creates a pull request in `workspace`/`repo_slug` with the given JSON body.
    /// Returns the created pull request, as raw JSON.
    pub fn create_pull_request(
        &self,
        workspace: &str,
        repo_slug: &str,
        body: &serde_json::Value,
    ) -> Result<serde_json::Value, ClientError> {
        self.post_json(&endpoints::path_pull_requests(workspace, repo_slug, None, None), body)
    }

    /// Adds a comment to the pull request identified by `id` in `workspace`/`repo_slug`.
    /// Returns the created comment, as raw JSON.
    pub fn create_pull_request_comment(
        &self,
        workspace: &str,
        repo_slug: &str,
        id: u64,
        body: &serde_json::Value,
    ) -> Result<serde_json::Value, ClientError> {
        self.post_json(&endpoints::path_pull_request_comments(workspace, repo_slug, id), body)
    }

    /// Approves the pull request identified by `id` in `workspace`/`repo_slug`.
    /// Returns the participant entry created by the approval, as raw JSON.
    pub fn approve_pull_request(&self, workspace: &str, repo_slug: &str, id: u64) -> Result<serde_json::Value, ClientError> {
        self.post_json(&endpoints::path_pull_request_approve(workspace, repo_slug, id), &serde_json::json!({}))
    }

    /// Removes the current user's approval from the pull request identified by `id`
    /// in `workspace`/`repo_slug`.
    pub fn unapprove_pull_request(&self, workspace: &str, repo_slug: &str, id: u64) -> Result<(), ClientError> {
        self.delete(&endpoints::path_pull_request_approve(workspace, repo_slug, id))
    }

    /// Declines the pull request identified by `id` in `workspace`/`repo_slug`.
    /// Returns the updated pull request, as raw JSON.
    pub fn decline_pull_request(&self, workspace: &str, repo_slug: &str, id: u64) -> Result<serde_json::Value, ClientError> {
        self.post_json(&endpoints::path_pull_request_decline(workspace, repo_slug, id), &serde_json::json!({}))
    }

    /// Merges the pull request identified by `id` in `workspace`/`repo_slug` with the
    /// given JSON body (`message`, `merge_strategy`, `close_source_branch`).
    /// Returns the merged pull request, as raw JSON.
    pub fn merge_pull_request(
        &self,
        workspace: &str,
        repo_slug: &str,
        id: u64,
        body: &serde_json::Value,
    ) -> Result<serde_json::Value, ClientError> {
        self.post_json(&endpoints::path_pull_request_merge(workspace, repo_slug, id), body)
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

    /// Deletes the repository identified by `workspace`/`repo_slug`. Permanent.
    pub fn delete_repository(&self, workspace: &str, repo_slug: &str) -> Result<(), ClientError> {
        self.delete(&endpoints::path_repository(workspace, repo_slug))
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

    fn delete(&self, path: &str) -> Result<(), ClientError> {
        let url = format!("{}{path}", self.base_url);

        let response = self
            .http
            .delete(&url)
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

        Ok(())
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
