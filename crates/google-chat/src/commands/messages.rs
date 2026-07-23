//! Handler for the `messages` command group (`messages list`, `messages send`).
//!
//! Delegates to `client::GoogleChatClient`. Maps any `ClientError` to
//! `CliError`, then prints the result as JSON (optionally filtered via
//! `--select`).

use crate::cli::MessagesCommand;
use crate::client::{self, ClientError};
use crate::context::{authenticated_client, print_json};
use crate::error::CliError;

/// Dispatches a `MessagesCommand` variant to the appropriate Chat API call.
///
/// `authenticated_client()` is called per-arm rather than once up front, so
/// that free, local validation (e.g. `Delete`'s `--confirm` check) runs
/// before the network round-trip a token refresh may require — a caller who
/// forgot `--confirm` finds out immediately instead of waiting on (and
/// possibly being confused by) an unrelated auth failure.
pub fn run(command: MessagesCommand, select: cli_fields::Select<'_>) -> Result<(), CliError> {
    match command {
        MessagesCommand::List {
            space,
            page_size,
            page_token,
            order_by,
        } => {
            let client = authenticated_client()?;
            let value = client
                .list_messages(&space, page_size, page_token.as_deref(), order_by.as_deref())
                .map_err(client_error_to_cli)?;
            print_json(&value, select)
        }
        MessagesCommand::Send { space, text } => {
            let client = authenticated_client()?;
            let value = client.create_message(&space, &text).map_err(client_error_to_cli)?;
            // Exempt: a single message object, fixed shape.
            print_json(&value, select.or_all())
        }
        MessagesCommand::Update { name, text } => {
            let client = authenticated_client()?;
            let value = client.update_message(&name, &text).map_err(client_error_to_cli)?;
            // Exempt: a single message object, fixed shape.
            print_json(&value, select.or_all())
        }
        MessagesCommand::Delete {
            name,
            confirm,
            delete_threaded_replies,
        } => {
            if !confirm {
                return Err(CliError::DeleteNotConfirmed { name });
            }
            let client = authenticated_client()?;
            client
                .delete_message(&name, delete_threaded_replies)
                .map_err(client_error_to_cli)?;
            let result = serde_json::json!({"deleted": true, "name": name});
            // Exempt: synthesized by us, always small.
            print_json(&result, select.or_all())
        }
    }
}

fn client_error_to_cli(e: ClientError) -> CliError {
    match e {
        client::ClientError::Request(reason) => CliError::ApiRequestFailed { reason },
        client::ClientError::Status { status, body } => CliError::ApiError { status, body },
    }
}
