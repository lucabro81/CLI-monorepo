//! Handler for the `project` command group.

use crate::cli::ProjectCommand;
use crate::context::{authenticated_client, client_error_to_cli, print_json};
use crate::error::CliError;

pub fn run(command: ProjectCommand, select: cli_fields::Select<'_>) -> Result<(), CliError> {
    match command {
        ProjectCommand::Search { query } => {
            let client = authenticated_client()?;
            let value = client.search_projects(&query).map_err(client_error_to_cli)?;
            print_json(&value, select)
        }
    }
}
