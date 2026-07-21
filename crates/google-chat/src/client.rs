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

    /// Lists messages in a space, as raw JSON (`{"messages": [...], "nextPageToken": "..."}`).
    /// `space` accepts either the bare space id or the full `spaces/{id}` resource name.
    /// Defaults to chronological order (the Chat API's own default, `createTime ASC`),
    /// which is what makes this usable as a context-recovery tool — pass
    /// `order_by: Some("createTime DESC")` to fetch the most recent messages first instead.
    pub fn list_messages(
        &self,
        space: &str,
        page_size: u32,
        page_token: Option<&str>,
        order_by: Option<&str>,
    ) -> Result<serde_json::Value, ClientError> {
        let parent = normalize_space_name(space);
        let mut pairs: Vec<(&str, String)> = vec![("pageSize", page_size.to_string())];
        if let Some(token) = page_token {
            pairs.push(("pageToken", token.to_string()));
        }
        if let Some(order) = order_by {
            pairs.push(("orderBy", order.to_string()));
        }
        let params = serde_urlencoded::to_string(&pairs)
            .map_err(|e| ClientError::Request(format!("failed to encode query params: {e}")))?;

        self.get_json(&format!("/{parent}/messages?{params}"))
    }

    /// Lists a space's memberships, as raw JSON (`{"memberships": [...], "nextPageToken": "..."}`).
    /// `space` accepts either the bare space id or the full `spaces/{id}` resource name. Each
    /// membership's `member.name` (`users/{id}`) is the same identifier `people_client::PeopleClient::get_user`
    /// resolves — used by `commands::spaces` to enrich each member with their People API profile.
    pub fn list_members(
        &self,
        space: &str,
        page_size: u32,
        page_token: Option<&str>,
    ) -> Result<serde_json::Value, ClientError> {
        let parent = normalize_space_name(space);
        let mut pairs: Vec<(&str, String)> = vec![("pageSize", page_size.to_string())];
        if let Some(token) = page_token {
            pairs.push(("pageToken", token.to_string()));
        }
        let params = serde_urlencoded::to_string(&pairs)
            .map_err(|e| ClientError::Request(format!("failed to encode query params: {e}")))?;

        self.get_json(&format!("/{parent}/members?{params}"))
    }

    /// Sends a plain-text message to a space and returns the created Message
    /// resource as raw JSON (includes its `name` field, needed to identify it
    /// later). `space` accepts either the bare space id or the full
    /// `spaces/{id}` resource name.
    pub fn create_message(&self, space: &str, text: &str) -> Result<serde_json::Value, ClientError> {
        let parent = normalize_space_name(space);
        let body = serde_json::json!({ "text": text });
        self.post_json(&format!("/{parent}/messages"), &body)
    }

    /// Creates a new space, or returns an existing one, via `spaces.setup`.
    /// One entry in `users` creates/finds a `DIRECT_MESSAGE` with that user —
    /// idempotent: if a DM already exists between the caller and that user,
    /// it is returned instead of creating a duplicate. Two or more entries
    /// create an unnamed `GROUP_CHAT`. Each entry accepts an email address,
    /// a bare Chat/People user id, or the full `users/{id}` resource name.
    pub fn setup_space(&self, users: &[String]) -> Result<serde_json::Value, ClientError> {
        let body = build_setup_space_body(users);
        self.post_json(endpoints::PATH_SPACES_SETUP, &body)
    }

    /// Permanently deletes a message. `name` is the full resource name
    /// (`spaces/{space}/messages/{message}`). `delete_threaded_replies`
    /// maps to the API's `force` query param — the request fails if the
    /// message has threaded replies and this is `false`.
    pub fn delete_message(&self, name: &str, delete_threaded_replies: bool) -> Result<(), ClientError> {
        let url = format!(
            "{}/{name}?force={delete_threaded_replies}",
            endpoints::CHAT_API_BASE_URL
        );

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

    fn get_json(&self, path: &str) -> Result<serde_json::Value, ClientError> {
        self.get_json_absolute(&format!("{}{path}", endpoints::CHAT_API_BASE_URL))
    }

    fn post_json(
        &self,
        path: &str,
        body: &serde_json::Value,
    ) -> Result<serde_json::Value, ClientError> {
        let url = format!("{}{path}", endpoints::CHAT_API_BASE_URL);

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

/// Normalizes a space identifier to the full `spaces/{id}` resource name
/// expected by the Chat API, accepting either form so a caller can paste the
/// bare id or the full `name` field straight from `spaces list` output.
pub(crate) fn normalize_space_name(space: &str) -> String {
    if space.starts_with("spaces/") {
        space.to_string()
    } else {
        format!("spaces/{space}")
    }
}

/// Normalizes a user identifier to the `users/{id}` resource name expected by
/// `spaces.setup`'s `member.name` field, accepting an email address, a bare
/// Chat/People user id, or the full `users/{id}` form already.
fn normalize_user_name(user: &str) -> String {
    if user.starts_with("users/") {
        user.to_string()
    } else {
        format!("users/{user}")
    }
}

/// Builds the `spaces.setup` request body: one `users` entry produces a
/// `DIRECT_MESSAGE` space with a single membership; two or more produce an
/// unnamed `GROUP_CHAT` with one membership per user.
fn build_setup_space_body(users: &[String]) -> serde_json::Value {
    let space_type = if users.len() == 1 { "DIRECT_MESSAGE" } else { "GROUP_CHAT" };
    let memberships: Vec<serde_json::Value> = users
        .iter()
        .map(|user| serde_json::json!({ "member": { "name": normalize_user_name(user), "type": "HUMAN" } }))
        .collect();

    serde_json::json!({
        "space": { "spaceType": space_type },
        "memberships": memberships,
    })
}

#[cfg(test)]
#[path = "tests/client_tests.rs"]
mod tests;
