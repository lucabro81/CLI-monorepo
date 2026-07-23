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
    /// A pull subscription with this name already exists, but its `topic`
    /// and/or `filter` differ from what was requested — both are immutable
    /// after creation, so this can't be silently reconciled the way a
    /// matching 409 is.
    ConfigMismatch { subscription: String, reason: String },
}

impl std::fmt::Display for EventsClientError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EventsClientError::Request(msg) => write!(f, "request failed: {msg}"),
            EventsClientError::Status { status, body } => {
                write!(f, "API returned status {status}: {body}")
            }
            EventsClientError::ConfigMismatch { subscription, reason } => {
                write!(f, "subscription {subscription} already exists with different configuration: {reason}")
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
            EventsClientError::ConfigMismatch { subscription, reason } => {
                CliError::PubsubSubscriptionMismatch { subscription, reason }
            }
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
            // Never produced by the Workspace Events calls this maps errors
            // for (only ensure_pubsub_subscription produces it) — mapped the
            // same way as into_pubsub_error for exhaustiveness.
            EventsClientError::ConfigMismatch { subscription, reason } => {
                CliError::PubsubSubscriptionMismatch { subscription, reason }
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

    /// Ensures a pull subscription named `subscription` exists on `topic`
    /// with the given `filter` (if any), creating it if missing. `filter`,
    /// if given, is a Pub/Sub filter expression (e.g.
    /// `hasPrefix(attributes.ce-subject, ...)`) applied to the
    /// subscription so only matching messages are delivered — see Pub/Sub's
    /// subscription filter docs.
    ///
    /// If a subscription with this name already exists (Pub/Sub returns 409
    /// `ALREADY_EXISTS`), its actual `topic`/`filter` are fetched and
    /// compared against what was requested: a match is treated as success
    /// (the caller does not need to check beforehand whether it exists), but
    /// a mismatch is an error — both fields are immutable after creation, so
    /// a differing request can never actually take effect on the existing
    /// subscription.
    ///
    /// `subscription` and `topic` are full resource names
    /// (`projects/{project}/subscriptions/{subscription}` and
    /// `projects/{project}/topics/{topic}`).
    pub fn ensure_pubsub_subscription(
        &self,
        subscription: &str,
        topic: &str,
        filter: Option<&str>,
    ) -> Result<(), EventsClientError> {
        let url = format!("{}/{subscription}", endpoints::PUBSUB_API_BASE_URL);
        let body = build_pubsub_subscription_body(topic, filter);

        let response = self
            .http
            .put(&url)
            .bearer_auth(&self.access_token)
            .header("Accept", "application/json")
            .json(&body)
            .send()
            .map_err(|e| EventsClientError::Request(e.to_string()))?;

        let status = response.status();
        if status.is_success() {
            return Ok(());
        }
        if status.as_u16() == 409 {
            let existing = self.get_pubsub_subscription(subscription)?;
            return match subscription_config_mismatch(&existing, topic, filter) {
                Some(reason) => Err(EventsClientError::ConfigMismatch {
                    subscription: subscription.to_string(),
                    reason,
                }),
                None => Ok(()),
            };
        }
        Err(EventsClientError::Status {
            status: status.as_u16(),
            body: response.text().unwrap_or_default(),
        })
    }

    /// Fetches the current state of a Pub/Sub pull subscription. `subscription`
    /// is a full resource name (`projects/{project}/subscriptions/{subscription}`).
    pub fn get_pubsub_subscription(&self, subscription: &str) -> Result<serde_json::Value, EventsClientError> {
        let url = format!("{}/{subscription}", endpoints::PUBSUB_API_BASE_URL);

        let response = self
            .http
            .get(&url)
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

    /// Fetches a Workspace Events subscription by resource name
    /// (`subscriptions/{id}`). Returns the subscription resource as raw JSON.
    pub fn get_subscription(&self, name: &str) -> Result<serde_json::Value, EventsClientError> {
        let url = format!("{}/{name}", endpoints::WORKSPACE_EVENTS_API_BASE_URL);

        let response = self
            .http
            .get(&url)
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

    /// Lists Workspace Events subscriptions matching `event_types` (OR'd
    /// together — the API requires at least one) and, if `space` is given,
    /// additionally restricted to that space's `target_resource` (see
    /// `build_list_filter`). Returns raw JSON (`{"subscriptions": [...],
    /// "nextPageToken": "..."}`).
    pub fn list_subscriptions(
        &self,
        event_types: &[String],
        space: Option<&str>,
        page_size: u32,
        page_token: Option<&str>,
    ) -> Result<serde_json::Value, EventsClientError> {
        let filter = build_list_filter(event_types, space);
        let mut pairs: Vec<(&str, String)> = vec![("filter", filter), ("pageSize", page_size.to_string())];
        if let Some(token) = page_token {
            pairs.push(("pageToken", token.to_string()));
        }
        let params = serde_urlencoded::to_string(&pairs)
            .map_err(|e| EventsClientError::Request(format!("failed to encode query params: {e}")))?;
        let url = format!("{}/subscriptions?{params}", endpoints::WORKSPACE_EVENTS_API_BASE_URL);

        let response = self
            .http
            .get(&url)
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

/// Builds the Pub/Sub `subscriptions.create` request body. Includes a
/// `filter` key only when `filter` is `Some` — Pub/Sub treats an absent
/// `filter` as "no filtering", so omitting the key (rather than sending an
/// empty string) is required to preserve default behavior when the caller
/// doesn't pass `--message-filter`.
fn build_pubsub_subscription_body(topic: &str, filter: Option<&str>) -> serde_json::Value {
    match filter {
        Some(filter) => serde_json::json!({ "topic": topic, "filter": filter }),
        None => serde_json::json!({ "topic": topic }),
    }
}

/// Builds the `subscriptions.list` filter string from typed CLI flags: event
/// types combined with `OR`, optionally `AND`'d with a `target_resource`
/// clause scoping to one space. Parenthesizes the `OR` clause when there is
/// more than one event type and a `target_resource` clause follows it, since
/// `AND` binds tighter than a bare `OR` in the filter grammar. Mirrors the
/// syntax documented at
/// <https://developers.google.com/workspace/events/guides/list-subscriptions>.
fn build_list_filter(event_types: &[String], space: Option<&str>) -> String {
    let event_clause = event_types
        .iter()
        .map(|t| format!("event_types:\"{t}\""))
        .collect::<Vec<_>>()
        .join(" OR ");

    let Some(space) = space else {
        return event_clause;
    };

    let target_resource = format!("//chat.googleapis.com/{}", normalize_space_name(space));
    let event_clause = if event_types.len() > 1 {
        format!("({event_clause})")
    } else {
        event_clause
    };
    format!("{event_clause} AND target_resource=\"{target_resource}\"")
}

/// Compares an existing Pub/Sub subscription resource (as returned by
/// `get_pubsub_subscription`) against a requested `topic`/`filter`. Returns
/// `None` if they match, or `Some(reason)` describing which field(s) differ
/// if they don't — both fields are immutable after creation, so a mismatch
/// here means the requested configuration can never take effect on the
/// existing subscription.
fn subscription_config_mismatch(existing: &serde_json::Value, topic: &str, filter: Option<&str>) -> Option<String> {
    let existing_topic = existing["topic"].as_str().unwrap_or("");
    let existing_filter = existing["filter"].as_str().unwrap_or("");
    let requested_filter = filter.unwrap_or("");

    let mut mismatches = Vec::new();
    if existing_topic != topic {
        mismatches.push(format!("topic: existing \"{existing_topic}\" vs requested \"{topic}\""));
    }
    if existing_filter != requested_filter {
        mismatches.push(format!(
            "filter: existing \"{existing_filter}\" vs requested \"{requested_filter}\""
        ));
    }

    if mismatches.is_empty() {
        None
    } else {
        Some(mismatches.join("; "))
    }
}

#[cfg(test)]
#[path = "tests/events_client_tests.rs"]
mod tests;
