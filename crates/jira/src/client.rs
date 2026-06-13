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
use crate::endpoints;

/// A workflow transition available for an issue.
#[derive(Debug, Deserialize)]
pub struct JiraTransition {
    pub id: String,
    pub name: String,
}

/// Error returned by `JiraClient` methods.
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
                write!(f, "Jira returned status {status}: {body}")
            }
        }
    }
}

/// Blocking HTTP client for the Jira REST API v3.
pub struct JiraClient {
    base_url: String,
    access_token: String,
    http: reqwest::blocking::Client,
}

impl JiraClient {
    /// Builds a client from stored credentials. The base URL is derived from `cloud_id`.
    pub fn new(credentials: &Credentials) -> Self {
        Self {
            base_url: format!("{}/{}", endpoints::JIRA_API_BASE_URL, credentials.cloud_id),
            access_token: credentials.access_token.clone(),
            http: reqwest::blocking::Client::new(),
        }
    }

    /// Fetches a Jira issue by key and returns the raw JSON response.
    pub fn get_issue(&self, key: &str) -> Result<serde_json::Value, ClientError> {
        self.get_json(&endpoints::issue_path(key))
    }

    /// Returns the currently authenticated user as raw JSON.
    pub fn get_myself(&self) -> Result<serde_json::Value, ClientError> {
        self.get_json(endpoints::PATH_MYSELF)
    }

    /// Returns the permissions granted to the current user/token, restricted to
    /// `permission_keys` (e.g. `["CREATE_ISSUES", "BROWSE_PROJECTS"]`). If
    /// `project_key` is `Some`, the permissions are evaluated in that project's
    /// context; if `None`, the result reflects global (instance-level) grants.
    /// The result is the raw Jira response:
    /// `{"permissions": {"<KEY>": {"havePermission": bool, ...}}}`.
    pub fn get_my_permissions(
        &self,
        permission_keys: &[&str],
        project_key: Option<&str>,
    ) -> Result<serde_json::Value, ClientError> {
        let mut pairs: Vec<(&str, String)> = vec![("permissions", permission_keys.join(","))];
        if let Some(key) = project_key {
            pairs.push(("projectKey", key.to_string()));
        }
        let query = serde_urlencoded::to_string(&pairs)
            .map_err(|e| ClientError::Request(format!("failed to encode query params: {e}")))?;
        self.get_json(&format!("{}?{query}", endpoints::PATH_MY_PERMISSIONS))
    }

    /// Returns the key of every project visible to the current user, paginating
    /// through `/rest/api/3/project/search` until `isLast` is true.
    pub fn list_projects(&self) -> Result<Vec<String>, ClientError> {
        let mut keys = Vec::new();
        let mut start_at = 0u32;

        loop {
            let page = self.get_json(&format!(
                "{}?startAt={start_at}&maxResults=50",
                endpoints::PATH_PROJECT_SEARCH
            ))?;

            let values = page["values"].as_array().cloned().unwrap_or_default();
            for value in &values {
                if let Some(key) = value["key"].as_str() {
                    keys.push(key.to_string());
                }
            }

            if page["isLast"].as_bool().unwrap_or(true) || values.is_empty() {
                break;
            }
            start_at += u32::try_from(values.len()).unwrap_or(u32::MAX);
        }

        Ok(keys)
    }

    /// Returns the project roles defined for `project_key` as `(role name, role URL)`
    /// pairs, via `/rest/api/3/project/<key>/role`.
    pub fn get_project_roles(&self, project_key: &str) -> Result<Vec<(String, String)>, ClientError> {
        let value = self.get_json(&endpoints::project_roles_path(project_key))?;
        let roles = value
            .as_object()
            .map(|map| {
                map.iter()
                    .filter_map(|(name, url)| url.as_str().map(|url| (name.clone(), url.to_string())))
                    .collect()
            })
            .unwrap_or_default();
        Ok(roles)
    }

    /// Returns the account IDs of the user actors assigned to the project role at
    /// `role_url` (an absolute URL, as returned by [`Self::get_project_roles`]).
    /// Group actors are not resolved and are skipped.
    pub fn get_role_actor_account_ids(&self, role_url: &str) -> Result<Vec<String>, ClientError> {
        let value = self.get_json_absolute(role_url)?;
        let actors = value["actors"].as_array().cloned().unwrap_or_default();
        Ok(actors
            .iter()
            .filter_map(|actor| actor["actorUser"]["accountId"].as_str().map(str::to_string))
            .collect())
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
        self.post_json(&endpoints::issue_comment_path(key), &body)
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
            pairs.push(("nextPageToken", token.to_string()));
        }
        let params = serde_urlencoded::to_string(&pairs)
            .map_err(|e| ClientError::Request(format!("failed to encode query params: {e}")))?;

        self.get_json(&format!("{}?{params}", endpoints::PATH_SEARCH_JQL))
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

        self.post_json(endpoints::PATH_ISSUE, &serde_json::json!({"fields": fields}))
    }

    /// Permanently deletes an issue by key.
    /// Set `delete_subtasks` to true if the issue has subtasks; Jira returns 400 otherwise.
    /// Returns `()` on success (Jira responds with 204 No Content).
    pub fn delete_issue(&self, key: &str, delete_subtasks: bool) -> Result<(), ClientError> {
        let url = format!(
            "{}{}?deleteSubtasks={}",
            self.base_url,
            endpoints::issue_path(key),
            delete_subtasks
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
        self.get_json(&endpoints::issue_transitions_path(key))
    }

    /// Returns the list of workflow transitions available for an issue in its current state.
    pub fn get_transitions(&self, key: &str) -> Result<Vec<JiraTransition>, ClientError> {
        #[derive(Deserialize)]
        struct TransitionsResponse {
            transitions: Vec<JiraTransition>,
        }

        let value = self.get_json(&endpoints::issue_transitions_path(key))?;
        let resp: TransitionsResponse = serde_json::from_value(value)
            .map_err(|e| ClientError::Request(format!("failed to parse transitions: {e}")))?;
        Ok(resp.transitions)
    }

    /// Executes a workflow transition on an issue by transition ID.
    /// Returns `()` on success (Jira responds with 204 No Content).
    pub fn apply_transition(&self, key: &str, transition_id: &str) -> Result<(), ClientError> {
        let body = serde_json::json!({"transition": {"id": transition_id}});
        let url = format!("{}{}", self.base_url, endpoints::issue_transitions_path(key));

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
        let url = format!(
            "{}{}",
            self.base_url,
            endpoints::issue_comment_id_path(key, comment_id)
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
        self.get_json_absolute(&format!("{}{path}", self.base_url))
    }

    /// Like [`Self::get_json`], but `url` is used as-is rather than appended to
    /// `base_url`. Used for endpoints that return absolute URLs to follow, such
    /// as project role actor lists.
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
