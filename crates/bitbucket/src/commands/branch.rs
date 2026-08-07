//! Handler for the `branch` command group.

use crate::cli::BranchCommand;
use crate::context::{authenticated_client, print_json, split_repository};
use crate::error::CliError;

/// Dispatches a `BranchCommand` variant to the appropriate Bitbucket API call.
pub fn run(command: BranchCommand, select: cli_fields::Select<'_>) -> Result<(), CliError> {
    match command {
        BranchCommand::List { repository, page } => {
            let (workspace, repo_slug) = split_repository(&repository)?;
            let value = authenticated_client()?
                .list_branches(workspace, repo_slug, page)
                .map_err(|e| CliError::ApiRequestFailed {
                    reason: e.to_string(),
                })?;
            print_json(&value, select)
        }
        BranchCommand::Create { repository, name, target } => {
            let (workspace, repo_slug) = split_repository(&repository)?;
            let body = serde_json::json!({
                "name": name,
                "target": { "hash": target }
            });
            let value = authenticated_client()?
                .create_branch(workspace, repo_slug, &body)
                .map_err(|e| CliError::ApiRequestFailed {
                    reason: e.to_string(),
                })?;
            // Exempt: a single branch object, fixed shape.
            print_json(&value, select.or_all())
        }
    }
}
