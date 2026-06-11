//! Handler for the `repo` command group.

use crate::cli::RepoCommand;
use crate::context::{authenticated_client, print_json};
use crate::error::CliError;

/// Dispatches a `RepoCommand` variant to the appropriate Bitbucket API call.
pub fn run(command: RepoCommand, select: &[&str]) -> Result<(), CliError> {
    match command {
        RepoCommand::Get { repository } => {
            let (workspace, repo_slug) = split_repository(&repository)?;
            let value = authenticated_client()?
                .get_repository(workspace, repo_slug)
                .map_err(|e| CliError::ApiRequestFailed {
                    reason: e.to_string(),
                })?;
            print_json(&value, select)
        }
    }
}

/// Splits `workspace/repo_slug` into its two parts, rejecting any other shape.
fn split_repository(repository: &str) -> Result<(&str, &str), CliError> {
    match repository.split_once('/') {
        Some((workspace, repo_slug)) if !workspace.is_empty() && !repo_slug.is_empty() => {
            Ok((workspace, repo_slug))
        }
        _ => Err(CliError::InvalidRepository {
            value: repository.to_string(),
        }),
    }
}

#[cfg(test)]
#[path = "repo_tests.rs"]
mod tests;
