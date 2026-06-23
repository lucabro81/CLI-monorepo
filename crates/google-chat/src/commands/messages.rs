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
pub fn run(command: MessagesCommand, select: &[&str]) -> Result<(), CliError> {
    let client = authenticated_client()?;
    match command {
        MessagesCommand::List {
            space,
            page_size,
            page_token,
            order_by,
        } => {
            let value = client
                .list_messages(&space, page_size, page_token.as_deref(), order_by.as_deref())
                .map_err(client_error_to_cli)?;
            print_json(&value, select)
        }
        MessagesCommand::Send { space, text } => {
            let value = client.create_message(&space, &text).map_err(client_error_to_cli)?;
            print_json(&value, select)
        }
    }
}

fn client_error_to_cli(e: ClientError) -> CliError {
    match e {
        client::ClientError::Request(reason) => CliError::ApiRequestFailed { reason },
        client::ClientError::Status { status, body } => CliError::ApiError { status, body },
    }
}
