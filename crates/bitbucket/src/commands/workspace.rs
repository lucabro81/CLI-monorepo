//! Handler for the `workspace` command group.

use crate::cli::WorkspaceCommand;
use crate::context::{authenticated_client, print_json};
use crate::error::CliError;

/// Dispatches a `WorkspaceCommand` variant to the appropriate Bitbucket API call.
pub fn run(command: WorkspaceCommand, select: cli_fields::Select<'_>) -> Result<(), CliError> {
    match command {
        WorkspaceCommand::Members { workspace, page } => {
            let value = authenticated_client()?
                .list_workspace_members(&workspace, page)
                .map_err(|e| CliError::ApiRequestFailed {
                    reason: e.to_string(),
                })?;
            print_json(&value, select)
        }
    }
}
