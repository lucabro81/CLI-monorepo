//! Handler for the `space` command group (`list`).

use crate::cli::SpaceCommand;
use crate::context::{authenticated_client, client_error_to_cli, print_json};
use crate::error::CliError;

pub fn run(command: SpaceCommand, select: cli_fields::Select<'_>) -> Result<(), CliError> {
    match command {
        SpaceCommand::List { limit, cursor } => {
            let value = authenticated_client()?
                .list_spaces(limit, cursor.as_deref())
                .map_err(client_error_to_cli)?;
            print_json(&value, select)
        }
    }
}
