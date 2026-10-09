//! Handler for the `repo` command group.

use serde_json::{json, Value};

use crate::cli::RepoCommand;
use crate::auth::Identity;
use crate::context::{authenticated_client, print_json, split_repository, client_error_to_cli};
use crate::error::CliError;

/// Dispatches a `RepoCommand` variant to the appropriate Bitbucket API call.
pub fn run(command: RepoCommand, select: cli_fields::Select<'_>, identity: &Identity) -> Result<(), CliError> {
    match command {
        RepoCommand::Get { repository } => {
            let (workspace, repo_slug) = split_repository(&repository)?;
            let value = authenticated_client(identity)?
                .get_repository(workspace, repo_slug)
                .map_err(client_error_to_cli)?;
            // Exempt: a single repository object, fixed shape.
            print_json(&value, select.or_all())
        }
        RepoCommand::List { workspace, page } => {
            let value = authenticated_client(identity)?
                .list_repositories(&workspace, page)
                .map_err(client_error_to_cli)?;
            print_json(&value, select)
        }
        RepoCommand::Create { repository, description, private, project } => {
            let (workspace, repo_slug) = split_repository(&repository)?;
            let body = build_create_body(description, private, project);
            let value = authenticated_client(identity)?
                .create_repository(workspace, repo_slug, &body)
                .map_err(client_error_to_cli)?;
            // Exempt: a single repository object, fixed shape.
            print_json(&value, select.or_all())
        }
        RepoCommand::Delete { repository, confirm } => {
            if !confirm {
                return Err(CliError::RepoDeleteNotConfirmed { repository });
            }
            let (workspace, repo_slug) = split_repository(&repository)?;
            authenticated_client(identity)?
                .delete_repository(workspace, repo_slug)
                .map_err(client_error_to_cli)?;
            // Exempt: synthesized by us, always small.
            print_json(&json!({"deleted": true, "repository": repository}), select.or_all())
        }
    }
}

/// Builds the `POST /2.0/repositories/{workspace}/{repo_slug}` request body.
/// `scm` is always `"git"`; other fields are included only if set.
fn build_create_body(description: Option<String>, private: bool, project: Option<String>) -> Value {
    let mut body = json!({"scm": "git"});
    let map = body.as_object_mut().unwrap_or_else(|| {
        unreachable!("body is always constructed as a JSON object literal above")
    });

    if let Some(description) = description {
        map.insert("description".to_string(), Value::String(description));
    }
    if private {
        map.insert("is_private".to_string(), Value::Bool(true));
    }
    if let Some(project) = project {
        map.insert("project".to_string(), json!({"key": project}));
    }

    body
}

#[cfg(test)]
#[path = "../tests/commands/repo_tests.rs"]
mod tests;
