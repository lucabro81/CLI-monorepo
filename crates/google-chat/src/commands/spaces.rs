//! Handler for the `spaces` command group (`spaces list`).
//!
//! Delegates to `client::GoogleChatClient`. Maps any `ClientError` to
//! `CliError`, then prints the result as JSON (optionally filtered via
//! `--select`).

use crate::cli::SpacesCommand;
use crate::client::{self, ClientError};
use crate::context::{authenticated_client, print_json};
use crate::error::CliError;

/// Dispatches a `SpacesCommand` variant to the appropriate Chat API call.
pub fn run(command: SpacesCommand, select: &[&str]) -> Result<(), CliError> {
    let client = authenticated_client()?;
    match command {
        SpacesCommand::List { page_size, page_token } => {
            let value = client
                .list_spaces(page_size, page_token.as_deref())
                .map_err(client_error_to_cli)?;
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
