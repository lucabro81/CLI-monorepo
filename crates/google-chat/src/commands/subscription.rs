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
pub fn run(command: SubscriptionCommand, select: cli_fields::Select<'_>) -> Result<(), CliError> {
    match command {
        SubscriptionCommand::Create {
            space,
            topic,
            pubsub_subscription,
            event_type,
            message_filter,
            allow_unfiltered,
        } => {
            require_message_filter(message_filter.as_deref(), allow_unfiltered, &pubsub_subscription)?;
            let credentials = authenticated_credentials()?;
            let client = EventsClient::new(&credentials.access_token);
            client
                .ensure_pubsub_subscription(&pubsub_subscription, &topic, message_filter.as_deref())
                .map_err(EventsClientError::into_pubsub_error)?;
            let value = client
                .create_workspace_events_subscription(&space, &event_type, &topic)
                .map_err(EventsClientError::into_workspace_events_error)?;
            // Exempt: a single subscription object, fixed shape.
            print_json(&value, select.or_all())
        }
        SubscriptionCommand::Delete { name } => {
            let credentials = authenticated_credentials()?;
            let client = EventsClient::new(&credentials.access_token);
            let value = client
                .delete_subscription(&name)
                .map_err(EventsClientError::into_workspace_events_error)?;
            // Exempt: a small confirmation object, fixed shape.
            print_json(&value, select.or_all())
        }
    }
}

/// Enforces that `subscription create` was given an explicit delivery scope:
/// either a `--message-filter` expression, or an explicit `--allow-unfiltered`
/// opt-out. Mirrors the `--select`/`--select-all` "required unless explicitly
/// confirmed" pattern — an unfiltered subscription silently flooding an
/// agent's `listen` stream with events from unrelated spaces is the same
/// class of footgun as an unbounded JSON dump.
fn require_message_filter(
    message_filter: Option<&str>,
    allow_unfiltered: bool,
    pubsub_subscription: &str,
) -> Result<(), CliError> {
    if message_filter.is_none() && !allow_unfiltered {
        return Err(CliError::MessageFilterRequired {
            pubsub_subscription: pubsub_subscription.to_string(),
        });
    }
    Ok(())
}

#[cfg(test)]
#[path = "../tests/commands/subscription_tests.rs"]
mod tests;
