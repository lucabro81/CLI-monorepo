//! Handler for the `pr` command group.

use crate::cli::PrCommand;
use crate::context::{authenticated_client, print_json, split_repository};
use crate::error::CliError;

/// Dispatches a `PrCommand` variant to the appropriate Bitbucket API call.
pub fn run(command: PrCommand, select: &[&str]) -> Result<(), CliError> {
    match command {
        PrCommand::Get { repository, id } => {
            let (workspace, repo_slug) = split_repository(&repository)?;
            let value = authenticated_client()?
                .get_pull_request(workspace, repo_slug, id)
                .map_err(|e| CliError::ApiRequestFailed {
                    reason: e.to_string(),
                })?;
            print_json(&value, select)
        }
        PrCommand::List { repository, state, page } => {
            let (workspace, repo_slug) = split_repository(&repository)?;
            let value = authenticated_client()?
                .list_pull_requests(workspace, repo_slug, state.as_deref(), page)
                .map_err(|e| CliError::ApiRequestFailed {
                    reason: e.to_string(),
                })?;
            print_json(&value, select)
        }
    }
}
