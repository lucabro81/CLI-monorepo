//! Jira REST API v3 HTTP client.
//!
//! `JiraClient` wraps a blocking `reqwest` client pre-configured with a
//! `Bearer` token and the base URL `https://api.atlassian.com/ex/jira/<cloud_id>/`.
//! All methods return raw `serde_json::Value` so callers decide how much
//! structure to impose; the `--select` flag can then filter the output
//! client-side without requiring typed response structs for every endpoint.
//!
//! Private helpers `get_json` and `post_json` handle auth headers, URL
//! construction, and error mapping. DELETE operations build their URL inline.
//!
//! Search uses the current Atlassian endpoint `GET /rest/api/3/search/jql`
//! with cursor-based pagination (`nextPageToken`). The deprecated
//! `POST /rest/api/3/search` endpoint (410 Gone) is not used.

use serde::Deserialize;

use crate::auth::Credentials;

/// A workflow transition available for an issue.
#[derive(Debug, Deserialize)]
pub struct JiraTransition {
    pub id: String,
    pub name: String,
}

#[derive(Debug)]
pub enum ClientError {
    Request(String),
    Status { status: u16, body: String },
}

impl std::fmt::Display for ClientError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ClientError::Request(msg) => write!(f, "request failed: {msg}"),
            ClientError::Status { status, body } => {
                write!(f, "Jira returned status {status}: {body}")
            }
        }
    }
}

pub struct JiraClient {
    base_url: String,
    access_token: String,
    http: reqwest::blocking::Client,
}

impl JiraClient {
    pub fn new(credentials: &Credentials) -> Self {
        Self {
            base_url: format!("https://api.atlassian.com/ex/jira/{}", credentials.cloud_id),
            access_token: credentials.access_token.clone(),
            http: reqwest::blocking::Client::new(),
        }
    }

    /// Fetches a Jira issue by key and returns the raw JSON response.
    pub fn get_issue(&self, key: &str) -> Result<serde_json::Value, ClientError> {
        self.get_json(&format!("/rest/api/3/issue/{key}"))
    }

    /// Returns the currently authenticated user as raw JSON.
    pub fn get_myself(&self) -> Result<serde_json::Value, ClientError> {
        self.get_json("/rest/api/3/myself")
    }

    /// Adds a plain-text comment to an issue and returns the created comment as JSON.
    /// The text is wrapped in Jira's Atlassian Document Format (ADF) automatically.
    pub fn add_comment(&self, key: &str, text: &str) -> Result<serde_json::Value, ClientError> {
        let body = serde_json::json!({
            "body": {
                "type": "doc",
                "version": 1,
                "content": [{
                    "type": "paragraph",
                    "content": [{
                        "type": "text",
                        "text": text
                    }]
                }]
            }
        });
        self.post_json(&format!("/rest/api/3/issue/{key}/comment"), &body)
    }

    /// Searches issues using JQL and returns the raw Jira response.
    /// `fields` controls which Jira fields are included per issue (server-side);
    /// defaults to `*navigable` when `None`. Pass `*all` for every field.
    /// `page_token` is the cursor from a previous response's `nextPageToken` field.
    pub fn search_issues(
        &self,
        jql: &str,
        max_results: u32,
        page_token: Option<&str>,
        fields: Option<&str>,
    ) -> Result<serde_json::Value, ClientError> {
        let fields_value = fields.unwrap_or("*navigable");
        let mut pairs: Vec<(&str, String)> = vec![
            ("jql", jql.to_string()),
            ("maxResults", max_results.to_string()),
            ("fields", fields_value.to_string()),
        ];
        if let Some(token) = page_token {
            pairs.push(("pageToken", token.to_string()));
        }
        let params = serde_urlencoded::to_string(&pairs)
            .map_err(|e| ClientError::Request(format!("failed to encode query params: {e}")))?;

        self.get_json(&format!("/rest/api/3/search/jql?{params}"))
    }

    /// Creates a new issue and returns the Jira response (contains `id`, `key`, `self`).
    pub fn create_issue(
        &self,
        project: &str,
        issue_type: &str,
        summary: &str,
        description: Option<&str>,
        assignee: Option<&str>,
        priority: Option<&str>,
    ) -> Result<serde_json::Value, ClientError> {
        let mut fields = serde_json::json!({
            "project": {"key": project},
            "issuetype": {"name": issue_type},
            "summary": summary,
        });

        if let Some(text) = description {
            fields["description"] = serde_json::json!({
                "type": "doc",
                "version": 1,
                "content": [{"type": "paragraph", "content": [{"type": "text", "text": text}]}]
            });
        }
        if let Some(id) = assignee {
            fields["assignee"] = serde_json::json!({"accountId": id});
        }
        if let Some(p) = priority {
            fields["priority"] = serde_json::json!({"name": p});
        }

        self.post_json("/rest/api/3/issue", &serde_json::json!({"fields": fields}))
    }

    /// Permanently deletes an issue by key.
    /// Set `delete_subtasks` to true if the issue has subtasks; Jira returns 400 otherwise.
    /// Returns `()` on success (Jira responds with 204 No Content).
    pub fn delete_issue(&self, key: &str, delete_subtasks: bool) -> Result<(), ClientError> {
        let url = format!(
            "{}/rest/api/3/issue/{key}?deleteSubtasks={}",
            self.base_url, delete_subtasks
        );

        let response = self
            .http
            .delete(&url)
            .bearer_auth(&self.access_token)
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

    /// Returns the raw JSON response for available transitions (for display to the caller).
    /// Use this when you want to forward the full response as-is.
    pub fn list_transitions_json(&self, key: &str) -> Result<serde_json::Value, ClientError> {
        self.get_json(&format!("/rest/api/3/issue/{key}/transitions"))
    }

    /// Returns the list of workflow transitions available for an issue in its current state.
    pub fn get_transitions(&self, key: &str) -> Result<Vec<JiraTransition>, ClientError> {
        #[derive(Deserialize)]
        struct TransitionsResponse {
            transitions: Vec<JiraTransition>,
        }

        let value = self.get_json(&format!("/rest/api/3/issue/{key}/transitions"))?;
        let resp: TransitionsResponse = serde_json::from_value(value)
            .map_err(|e| ClientError::Request(format!("failed to parse transitions: {e}")))?;
        Ok(resp.transitions)
    }

    /// Executes a workflow transition on an issue by transition ID.
    /// Returns `()` on success (Jira responds with 204 No Content).
    pub fn apply_transition(&self, key: &str, transition_id: &str) -> Result<(), ClientError> {
        let body = serde_json::json!({"transition": {"id": transition_id}});
        let url = format!("{}/rest/api/3/issue/{key}/transitions", self.base_url);

        let response = self
            .http
            .post(&url)
            .bearer_auth(&self.access_token)
            .header("Accept", "application/json")
            .json(&body)
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

    /// Deletes a comment from an issue by its ID.
    /// Returns `()` on success (Jira responds with 204 No Content).
    pub fn delete_comment(&self, key: &str, comment_id: &str) -> Result<(), ClientError> {
        let url = format!("{}/rest/api/3/issue/{key}/comment/{comment_id}", self.base_url);

        let response = self
            .http
            .delete(&url)
            .bearer_auth(&self.access_token)
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

    fn post_json(
        &self,
        path: &str,
        body: &serde_json::Value,
    ) -> Result<serde_json::Value, ClientError> {
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
            .json::<serde_json::Value>()
            .map_err(|e| ClientError::Request(e.to_string()))
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
            .json::<serde_json::Value>()
            .map_err(|e| ClientError::Request(e.to_string()))
    }
}
