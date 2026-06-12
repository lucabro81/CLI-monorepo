//! Handler for the `pr` command group.

use serde_json::{json, Value};

use crate::cli::PrCommand;
use crate::context::{authenticated_client, print_json, split_repository};
use crate::error::CliError;

/// Dispatches a `PrCommand` variant to the appropriate Bitbucket API call.
pub fn run(command: PrCommand, select: &[&str]) -> Result<(), CliError> {
    match command {
        PrCommand::Create { repository, title, source, destination, description, close_source_branch } => {
            let (workspace, repo_slug) = split_repository(&repository)?;
            let body = build_create_body(&title, &source, destination, description, close_source_branch);
            let value = authenticated_client()?
                .create_pull_request(workspace, repo_slug, &body)
                .map_err(|e| CliError::ApiRequestFailed {
                    reason: e.to_string(),
                })?;
            print_json(&value, select)
        }
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

/// Builds the `POST /2.0/repositories/{workspace}/{repo_slug}/pullrequests` request body.
/// `title` and `source` are always included; other fields only if set.
fn build_create_body(title: &str, source: &str, destination: Option<String>, description: Option<String>, close_source_branch: bool) -> Value {
    let mut body = json!({
        "title": title,
        "source": {"branch": {"name": source}},
    });
    let map = body.as_object_mut().unwrap_or_else(|| {
        unreachable!("body is always constructed as a JSON object literal above")
    });

    if let Some(destination) = destination {
        map.insert("destination".to_string(), json!({"branch": {"name": destination}}));
    }
    if let Some(description) = description {
        map.insert("description".to_string(), Value::String(description));
    }
    if close_source_branch {
        map.insert("close_source_branch".to_string(), Value::Bool(true));
    }

    body
}

#[cfg(test)]
#[path = "pr_tests.rs"]
mod tests;
