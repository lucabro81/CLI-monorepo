//! Handler for the `pr` command group.

use serde_json::{json, Value};

use crate::cli::PrCommand;
use crate::context::{authenticated_client, print_json, split_repository};
use crate::error::CliError;

/// Dispatches a `PrCommand` variant to the appropriate Bitbucket API call.
pub fn run(command: PrCommand, select: cli_fields::Select<'_>) -> Result<(), CliError> {
    match command {
        PrCommand::Create { repository, title, source, destination, description, close_source_branch } => {
            let (workspace, repo_slug) = split_repository(&repository)?;
            let body = build_create_body(&title, &source, destination, description, close_source_branch);
            let value = authenticated_client()?
                .create_pull_request(workspace, repo_slug, &body)
                .map_err(|e| CliError::ApiRequestFailed {
                    reason: e.to_string(),
                })?;
            // Exempt: a single pull request object, fixed shape.
            print_json(&value, select.or_all())
        }
        PrCommand::Approve { repository, id } => {
            let (workspace, repo_slug) = split_repository(&repository)?;
            let value = authenticated_client()?
                .approve_pull_request(workspace, repo_slug, id)
                .map_err(|e| CliError::ApiRequestFailed {
                    reason: e.to_string(),
                })?;
            // Exempt: a small approval object.
            print_json(&value, select.or_all())
        }
        PrCommand::Unapprove { repository, id } => {
            let (workspace, repo_slug) = split_repository(&repository)?;
            authenticated_client()?
                .unapprove_pull_request(workspace, repo_slug, id)
                .map_err(|e| CliError::ApiRequestFailed {
                    reason: e.to_string(),
                })?;
            // Exempt: synthesized by us, always small.
            print_json(&json!({"unapproved": true, "id": id}), select.or_all())
        }
        PrCommand::Decline { repository, id, confirm } => {
            if !confirm {
                return Err(CliError::DeclineNotConfirmed { repository, id });
            }
            let (workspace, repo_slug) = split_repository(&repository)?;
            let value = authenticated_client()?
                .decline_pull_request(workspace, repo_slug, id)
                .map_err(|e| CliError::ApiRequestFailed {
                    reason: e.to_string(),
                })?;
            // Exempt: a single pull request object, fixed shape.
            print_json(&value, select.or_all())
        }
        PrCommand::Merge { repository, id, message, merge_strategy, close_source_branch, confirm } => {
            if !confirm {
                return Err(CliError::MergeNotConfirmed { repository, id });
            }
            let (workspace, repo_slug) = split_repository(&repository)?;
            let body = build_merge_body(message, merge_strategy, close_source_branch);
            let value = authenticated_client()?
                .merge_pull_request(workspace, repo_slug, id, &body)
                .map_err(|e| CliError::ApiRequestFailed {
                    reason: e.to_string(),
                })?;
            // Exempt: a single pull request object, fixed shape.
            print_json(&value, select.or_all())
        }
        PrCommand::Comment { repository, id, content, path, line } => {
            let (workspace, repo_slug) = split_repository(&repository)?;
            let inline = validate_inline_location(path, line)?;
            let body = build_comment_body(&content, inline);
            let value = authenticated_client()?
                .create_pull_request_comment(workspace, repo_slug, id, &body)
                .map_err(|e| CliError::ApiRequestFailed {
                    reason: e.to_string(),
                })?;
            // Exempt: a single comment object, fixed shape.
            print_json(&value, select.or_all())
        }
        PrCommand::Get { repository, id } => {
            let (workspace, repo_slug) = split_repository(&repository)?;
            let value = authenticated_client()?
                .get_pull_request(workspace, repo_slug, id)
                .map_err(|e| CliError::ApiRequestFailed {
                    reason: e.to_string(),
                })?;
            // Exempt: a single pull request object, fixed shape.
            print_json(&value, select.or_all())
        }
        PrCommand::Diff { repository, id, context, path } => {
            let (workspace, repo_slug) = split_repository(&repository)?;
            let diff = authenticated_client()?
                .get_pull_request_diff(workspace, repo_slug, id, context, path.as_deref())
                .map_err(|e| CliError::ApiRequestFailed {
                    reason: e.to_string(),
                })?;
            print!("{diff}");
            Ok(())
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

/// Builds the `POST /2.0/repositories/{workspace}/{repo_slug}/pullrequests/{id}/merge`
/// request body. All fields are optional; an empty object lets Bitbucket apply its
/// repository defaults.
fn build_merge_body(message: Option<String>, merge_strategy: Option<String>, close_source_branch: bool) -> Value {
    let mut body = json!({});
    let map = body.as_object_mut().unwrap_or_else(|| {
        unreachable!("body is always constructed as a JSON object literal above")
    });

    if let Some(message) = message {
        map.insert("message".to_string(), Value::String(message));
    }
    if let Some(merge_strategy) = merge_strategy {
        map.insert("merge_strategy".to_string(), Value::String(merge_strategy));
    }
    if close_source_branch {
        map.insert("close_source_branch".to_string(), Value::Bool(true));
    }

    body
}

/// Validates `--path`/`--line` for `pr comment`: both or neither must be set.
/// Returns `Ok(Some((path, line)))` for an inline comment, `Ok(None)` for a general
/// comment, or `Err(CliError::InvalidInput)` if only one of the two is set.
fn validate_inline_location(path: Option<String>, line: Option<u64>) -> Result<Option<(String, u64)>, CliError> {
    match (path, line) {
        (Some(path), Some(line)) => Ok(Some((path, line))),
        (None, None) => Ok(None),
        _ => Err(CliError::InvalidInput {
            reason: "--path and --line must both be set for an inline comment, or both omitted for a general comment".to_string(),
        }),
    }
}

/// Builds the `POST /2.0/repositories/{workspace}/{repo_slug}/pullrequests/{id}/comments`
/// request body. `inline` adds an `inline` object with `path` and `to` (line number).
fn build_comment_body(content: &str, inline: Option<(String, u64)>) -> Value {
    let mut body = json!({
        "content": {"raw": content},
    });
    let map = body.as_object_mut().unwrap_or_else(|| {
        unreachable!("body is always constructed as a JSON object literal above")
    });

    if let Some((path, line)) = inline {
        map.insert("inline".to_string(), json!({"path": path, "to": line}));
    }

    body
}

#[cfg(test)]
#[path = "../tests/commands/pr_tests.rs"]
mod tests;
