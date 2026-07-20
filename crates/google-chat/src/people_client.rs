//! HTTP client for the Google People API v1 — used solely to resolve a
//! Google Chat user id (`users/{id}`, as it appears in a message's
//! `sender.name`) to that user's display name and email address.
//!
//! Kept separate from `client.rs`'s `GoogleChatClient`: this is a different
//! external API entirely (`people.googleapis.com`, not `chat.googleapis.com`),
//! with its own scope (`directory.readonly`) and its own resource-name
//! convention (`people/{id}`, not `users/{id}` — though the numeric id
//! portion is the same underlying Google account id for both). This exists
//! because the Chat API itself does not expose a user's display name under
//! either of this crate's auth modes: per Google's docs, "if your Chat app
//! authenticates as a user, the output for a User resource only populates
//! the user's name and type" — and both this crate's auth modes (service
//! account + domain-wide delegation, and 3LO) are "user auth" from the Chat
//! API's perspective, since neither requests the `chat.bot` scope. Uses the
//! same bearer access token as `GoogleChatClient`/`EventsClient` — different
//! scope, same OAuth identity, no separate auth flow. `directory.readonly`
//! is one of the scopes Google's People API docs list as sufficient for the
//! `emailAddresses` field, alongside `names`, so no additional scope or
//! re-consent is needed for email resolution.
//!
//! Only resolves users within the same Google Workspace domain as the
//! authenticated identity — a documented, accepted limitation (see
//! BACKLOG.md GCHAT-5).

use crate::endpoints;
use crate::error::CliError;

/// Error returned by `PeopleClient` methods. Same shape as `client::ClientError`.
#[derive(Debug)]
pub enum PeopleClientError {
    /// Network or serialization error — no HTTP response was received.
    Request(String),
    /// The server responded but with a non-2xx status code.
    Status { status: u16, body: String },
}

impl std::fmt::Display for PeopleClientError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PeopleClientError::Request(msg) => write!(f, "request failed: {msg}"),
            PeopleClientError::Status { status, body } => {
                write!(f, "People API returned status {status}: {body}")
            }
        }
    }
}

impl PeopleClientError {
    /// Maps an error from a People API call to the corresponding `CliError` variant.
    pub fn into_cli_error(self) -> CliError {
        match self {
            PeopleClientError::Request(reason) => CliError::PeopleApiRequestFailed { reason },
            PeopleClientError::Status { status, body } => CliError::PeopleApiError { status, body },
        }
    }
}

/// Blocking HTTP client for the Google People API v1.
pub struct PeopleClient {
    access_token: String,
    http: reqwest::blocking::Client,
}

impl PeopleClient {
    /// Builds a client from a raw OAuth access token — the same one used by
    /// `GoogleChatClient`/`EventsClient` for this identity, just a different
    /// scope on the same token.
    pub fn new(access_token: &str) -> Self {
        Self {
            access_token: access_token.to_string(),
            http: reqwest::blocking::Client::new(),
        }
    }

    /// Resolves a Google Chat user id to their People API profile
    /// (`personFields=names,emailAddresses`), as raw JSON. `user` accepts the
    /// bare numeric id, the full `users/{id}` resource name (as it appears in
    /// a message's `sender.name`), or the full `people/{id}` resource name.
    pub fn get_user(&self, user: &str) -> Result<serde_json::Value, PeopleClientError> {
        let resource = normalize_to_people_resource(user);
        let url = build_get_user_url(&resource);

        let response = self
            .http
            .get(&url)
            .bearer_auth(&self.access_token)
            .header("Accept", "application/json")
            .send()
            .map_err(|e| PeopleClientError::Request(e.to_string()))?;

        let status = response.status();
        if !status.is_success() {
            return Err(PeopleClientError::Status {
                status: status.as_u16(),
                body: response.text().unwrap_or_default(),
            });
        }

        response
            .json::<serde_json::Value>()
            .map_err(|e| PeopleClientError::Request(e.to_string()))
    }
}

/// Normalizes a Chat/People user identifier to the full `people/{id}`
/// resource name expected by the People API's `people.get`. Accepts a bare
/// numeric id, a `users/{id}` resource name (the format found in a message's
/// `sender.name`), or an already-correct `people/{id}` resource name — the
/// numeric id portion is the same underlying Google account id across all
/// three forms.
pub(crate) fn normalize_to_people_resource(user: &str) -> String {
    if let Some(id) = user.strip_prefix("users/") {
        format!("people/{id}")
    } else if user.starts_with("people/") {
        user.to_string()
    } else {
        format!("people/{user}")
    }
}

/// Builds the `people.get` request URL for an already-normalized
/// `people/{id}` resource, requesting both display names and email
/// addresses.
pub(crate) fn build_get_user_url(resource: &str) -> String {
    format!(
        "{}/{resource}?personFields=names,emailAddresses",
        endpoints::PEOPLE_API_BASE_URL
    )
}

#[cfg(test)]
#[path = "tests/people_client_tests.rs"]
mod tests;
