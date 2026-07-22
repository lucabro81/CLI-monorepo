//! Handler for the `user` command group.

use crate::cli::UserCommand;
use crate::context::{authenticated_client, print_json};
use crate::error::CliError;

/// Dispatches a `UserCommand` variant to the appropriate Admin API call.
pub fn run(command: UserCommand, select: cli_fields::Select<'_>) -> Result<(), CliError> {
    match command {
        UserCommand::Get { account_id } => {
            let value = authenticated_client()?
                .get_user(&account_id)
                .map_err(|e| CliError::ApiRequestFailed { reason: e.to_string() })?;
            // Exempt: a single profile object, fixed shape.
            print_json(&value, select.or_all())
        }
    }
}
