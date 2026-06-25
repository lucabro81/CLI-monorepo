//! REST client for the Workspace Events API (subscription management) and
//! the Pub/Sub API's subscription-admin surface.
//!
//! Kept separate from `client.rs`'s `GoogleChatClient`: same blocking-reqwest
//! shape, but a different pair of base URLs (`workspaceevents.googleapis.com`,
//! `pubsub.googleapis.com`) rather than `chat.googleapis.com`. Uses the same
//! bearer access token as `GoogleChatClient` — different scopes, same OAuth
//! identity, no separate auth flow.

use crate::client::normalize_space_name;
use crate::endpoints;
use crate::error::CliError;

/// Error returned by `EventsClient` methods. Same shape as `client::ClientError`.
#[derive(Debug)]
pub enum EventsClientError {
    /// Network or serialization error — no HTTP response was received.
    Request(String),
    /// The server responded but with a non-2xx, non-already-exists status code.
    Status { status: u16, body: String },
}

impl std::fmt::Display for EventsClientError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EventsClientError::Request(msg) => write!(f, "request failed: {msg}"),
            EventsClientError::Status { status, body } => {
                write!(f, "API returned status {status}: {body}")
            }
        }
    }
}

impl EventsClientError {
    /// Maps an error from a Pub/Sub admin call (`ensure_pubsub_subscription`)
    /// to the corresponding `CliError` variant.
    pub fn into_pubsub_error(self) -> CliError {
        match self {
            EventsClientError::Request(reason) => CliError::PubsubRequestFailed { reason },
            EventsClientError::Status { status, body } => CliError::PubsubApiError { status, body },
        }
    }

    /// Maps an error from a Workspace Events API call (`create_workspace_events_subscription`,
    /// `renew_subscription`) to the corresponding `CliError` variant.
    pub fn into_workspace_events_error(self) -> CliError {
        match self {
            EventsClientError::Request(reason) => CliError::WorkspaceEventsRequestFailed { reason },
            EventsClientError::Status { status, body } => {
                CliError::WorkspaceEventsApiError { status, body }
            }
        }
    }
}

/// Blocking HTTP client for the Workspace Events API and the Pub/Sub admin API.
pub struct EventsClient {
    access_token: String,
    http: reqwest::blocking::Client,
}

impl EventsClient {
    pub fn new(access_token: &str) -> Self {
        Self {
            access_token: access_token.to_string(),
            http: reqwest::blocking::Client::new(),
        }
    }

    /// Ensures a pull subscription named `subscription` exists on `topic`,
    /// creating it if missing. A subscription that already exists (Pub/Sub
    /// returns 409 `ALREADY_EXISTS`) is treated as success — the caller does
    /// not need to check beforehand whether it exists.
    ///
    /// `subscription` and `topic` are full resource names
    /// (`projects/{project}/subscriptions/{subscription}` and
    /// `projects/{project}/topics/{topic}`).
    pub fn ensure_pubsub_subscription(
        &self,
        subscription: &str,
        topic: &str,
    ) -> Result<(), EventsClientError> {
        let url = format!("{}/{subscription}", endpoints::PUBSUB_API_BASE_URL);
        let body = serde_json::json!({ "topic": topic });

        let response = self
            .http
            .put(&url)
            .bearer_auth(&self.access_token)
            .header("Accept", "application/json")
            .json(&body)
            .send()
            .map_err(|e| EventsClientError::Request(e.to_string()))?;

        let status = response.status();
        if status.is_success() || status.as_u16() == 409 {
            return Ok(());
        }
        Err(EventsClientError::Status {
            status: status.as_u16(),
            body: response.text().unwrap_or_default(),
        })
    }

    /// Creates a Workspace Events subscription delivering `event_types` for
    /// `space` to `topic`. `space` accepts either the bare space id or the
    /// full `spaces/{id}` resource name. Returns the created subscription
    /// resource as raw JSON.
    pub fn create_workspace_events_subscription(
        &self,
        space: &str,
        event_types: &[String],
        topic: &str,
    ) -> Result<serde_json::Value, EventsClientError> {
        let target_resource = format!(
            "//chat.googleapis.com/{}",
            normalize_space_name(space)
        );
        let body = build_subscription_body(&target_resource, event_types, topic);
        let url = format!("{}/subscriptions", endpoints::WORKSPACE_EVENTS_API_BASE_URL);

        let response = self
            .http
            .post(&url)
            .bearer_auth(&self.access_token)
            .header("Accept", "application/json")
            .json(&body)
            .send()
            .map_err(|e| EventsClientError::Request(e.to_string()))?;

        let status = response.status();
        if !status.is_success() {
            return Err(EventsClientError::Status {
                status: status.as_u16(),
                body: response.text().unwrap_or_default(),
            });
        }

        response
            .json::<serde_json::Value>()
            .map_err(|e| EventsClientError::Request(e.to_string()))
    }

    /// Renews `subscription` (a Workspace Events subscription resource name,
    /// `subscriptions/{id}`) to its maximum TTL, resetting its expiry clock.
    /// Returns the renewed subscription resource as raw JSON.
    pub fn renew_subscription(&self, subscription: &str) -> Result<serde_json::Value, EventsClientError> {
        let url = format!(
            "{}/{subscription}?updateMask=ttl",
            endpoints::WORKSPACE_EVENTS_API_BASE_URL
        );
        let body = serde_json::json!({ "ttl": "0s" });

        let response = self
            .http
            .patch(&url)
            .bearer_auth(&self.access_token)
            .header("Accept", "application/json")
            .json(&body)
            .send()
            .map_err(|e| EventsClientError::Request(e.to_string()))?;

        let status = response.status();
        if !status.is_success() {
            return Err(EventsClientError::Status {
                status: status.as_u16(),
                body: response.text().unwrap_or_default(),
            });
        }

        response
            .json::<serde_json::Value>()
            .map_err(|e| EventsClientError::Request(e.to_string()))
    }

    /// Deletes `subscription` (a Workspace Events subscription resource
    /// name, `subscriptions/{id}`), so no further events are delivered for
    /// it. Returns the operation resource as raw JSON.
    pub fn delete_subscription(&self, subscription: &str) -> Result<serde_json::Value, EventsClientError> {
        let url = format!("{}/{subscription}", endpoints::WORKSPACE_EVENTS_API_BASE_URL);

        let response = self
            .http
            .delete(&url)
            .bearer_auth(&self.access_token)
            .header("Accept", "application/json")
            .send()
            .map_err(|e| EventsClientError::Request(e.to_string()))?;

        let status = response.status();
        if !status.is_success() {
            return Err(EventsClientError::Status {
                status: status.as_u16(),
                body: response.text().unwrap_or_default(),
            });
        }

        response
            .json::<serde_json::Value>()
            .map_err(|e| EventsClientError::Request(e.to_string()))
    }
}

/// Builds the Workspace Events `subscriptions.create` request body.
fn build_subscription_body(
    target_resource: &str,
    event_types: &[String],
    topic: &str,
) -> serde_json::Value {
    serde_json::json!({
        "targetResource": target_resource,
        "eventTypes": event_types,
        "notificationEndpoint": { "pubsubTopic": topic },
        "payloadOptions": { "includeResource": true },
    })
}

#[cfg(test)]
#[path = "tests/events_client_tests.rs"]
mod tests;
