//! Handler for the `subscription` command group (`subscription create`,
//! `subscription delete`).
//!
//! Delegates to `events_client::EventsClient`, reusing the same OAuth access
//! token as `GoogleChatClient` (different scopes, same identity). Maps any
//! `EventsClientError` to `CliError` via its `into_*_error` helpers, then
//! prints the result as JSON (optionally filtered via `--select`).

use crate::cli::SubscriptionCommand;
use crate::context::{authenticated_credentials, print_json};
use crate::error::CliError;
use crate::events_client::{EventsClient, EventsClientError};

/// Dispatches a `SubscriptionCommand` variant to the appropriate API calls.
pub fn run(command: SubscriptionCommand, select: &[&str]) -> Result<(), CliError> {
    match command {
        SubscriptionCommand::Create {
            space,
            topic,
            pubsub_subscription,
            event_type,
        } => {
            let credentials = authenticated_credentials()?;
            let client = EventsClient::new(&credentials.access_token);
            client
                .ensure_pubsub_subscription(&pubsub_subscription, &topic)
                .map_err(EventsClientError::into_pubsub_error)?;
            let value = client
                .create_workspace_events_subscription(&space, &event_type, &topic)
                .map_err(EventsClientError::into_workspace_events_error)?;
            print_json(&value, select)
        }
        SubscriptionCommand::Delete { name } => {
            let credentials = authenticated_credentials()?;
            let client = EventsClient::new(&credentials.access_token);
            let value = client
                .delete_subscription(&name)
                .map_err(EventsClientError::into_workspace_events_error)?;
            print_json(&value, select)
        }
    }
}
