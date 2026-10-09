//! Handler for the `pr` command group.

use serde_json::{json, Value};

use crate::cli::PrCommand;
use crate::auth::Identity;
use crate::context::{authenticated_client, print_json, split_repository, client_error_to_cli};
use crate::error::CliError;

/// Dispatches a `PrCommand` variant to the appropriate Bitbucket API call.
pub fn run(command: PrCommand, select: cli_fields::Select<'_>, identity: &Identity) -> Result<(), CliError> {
    match command {
        PrCommand::Create { repository, title, source, destination, description, close_source_branch, reviewers, draft } => {
            let (workspace, repo_slug) = split_repository(&repository)?;
            let reviewer_uuids = split_reviewers(reviewers.as_deref());
            let body = build_create_body(&title, &source, destination, description, close_source_branch, reviewer_uuids, draft);
            let value = authenticated_client(identity)?
                .create_pull_request(workspace, repo_slug, &body)
                .map_err(client_error_to_cli)?;
            // Exempt: a single pull request object, fixed shape.
            print_json(&value, select.or_all())
        }
        PrCommand::Update { repository, id, title, description, destination, reviewers, draft, ready_for_review } => {
            let body = update_body_from_flags(title, description, destination, reviewers.as_deref(), draft, ready_for_review)?;
            run_update(&repository, id, &body, select, identity)
        }
        PrCommand::Approve { repository, id } => {
            let (workspace, repo_slug) = split_repository(&repository)?;
            let value = authenticated_client(identity)?
                .approve_pull_request(workspace, repo_slug, id)
                .map_err(client_error_to_cli)?;
            // Exempt: a small approval object.
            print_json(&value, select.or_all())
        }
        PrCommand::Unapprove { repository, id } => {
            let (workspace, repo_slug) = split_repository(&repository)?;
            authenticated_client(identity)?
                .unapprove_pull_request(workspace, repo_slug, id)
                .map_err(client_error_to_cli)?;
            // Exempt: synthesized by us, always small.
            print_json(&json!({"unapproved": true, "id": id}), select.or_all())
        }
        PrCommand::Decline { repository, id, confirm } => {
            if !confirm {
                return Err(CliError::DeclineNotConfirmed { repository, id });
            }
            let (workspace, repo_slug) = split_repository(&repository)?;
            let value = authenticated_client(identity)?
                .decline_pull_request(workspace, repo_slug, id)
                .map_err(client_error_to_cli)?;
            // Exempt: a single pull request object, fixed shape.
            print_json(&value, select.or_all())
        }
        PrCommand::Merge { repository, id, message, merge_strategy, close_source_branch, confirm } => {
            if !confirm {
                return Err(CliError::MergeNotConfirmed { repository, id });
            }
            let (workspace, repo_slug) = split_repository(&repository)?;
            let body = build_merge_body(message, merge_strategy, close_source_branch);
            let value = authenticated_client(identity)?
                .merge_pull_request(workspace, repo_slug, id, &body)
                .map_err(client_error_to_cli)?;
            // Exempt: a single pull request object, fixed shape.
            print_json(&value, select.or_all())
        }
        PrCommand::Comment { repository, id, content, path, line, parent } => {
            let inline = validate_inline_location(path, line)?;
            run_create_comment(&repository, id, &content, inline, parent, select, identity)
        }
        PrCommand::Get { repository, id } => {
            let (workspace, repo_slug) = split_repository(&repository)?;
            let value = authenticated_client(identity)?
                .get_pull_request(workspace, repo_slug, id)
                .map_err(client_error_to_cli)?;
            // Exempt: a single pull request object, fixed shape.
            print_json(&value, select.or_all())
        }
        PrCommand::Diff { repository, id, context, path } => {
            let (workspace, repo_slug) = split_repository(&repository)?;
            let diff = authenticated_client(identity)?
                .get_pull_request_diff(workspace, repo_slug, id, context, path.as_deref())
                .map_err(client_error_to_cli)?;
            print!("{diff}");
            Ok(())
        }
        PrCommand::ListComments { repository, id, page } => run_list_comments(&repository, id, page, select, identity),
        PrCommand::UpdateComment { repository, id, comment_id, content } => {
            run_update_comment(&repository, id, comment_id, &content, select, identity)
        }
        PrCommand::List { repository, state, page } => {
            let (workspace, repo_slug) = split_repository(&repository)?;
            let value = authenticated_client(identity)?
                .list_pull_requests(workspace, repo_slug, state.as_deref(), page)
                .map_err(client_error_to_cli)?;
            print_json(&value, select)
        }
    }
}

fn run_create_comment(
    repository: &str,
    id: u64,
    content: &str,
    inline: Option<(String, u64)>,
    parent: Option<u64>,
    select: cli_fields::Select<'_>,
    identity: &Identity,
) -> Result<(), CliError> {
    let (workspace, repo_slug) = split_repository(repository)?;
    let body = build_comment_body(content, inline, parent);
    let value = authenticated_client(identity)?
        .create_pull_request_comment(workspace, repo_slug, id, &body)
        .map_err(client_error_to_cli)?;
    // Exempt: a single comment object, fixed shape.
    print_json(&value, select.or_all())
}

fn run_list_comments(repository: &str, id: u64, page: Option<u32>, select: cli_fields::Select<'_>, identity: &Identity) -> Result<(), CliError> {
    let (workspace, repo_slug) = split_repository(repository)?;
    let value = authenticated_client(identity)?
        .list_pull_request_comments(workspace, repo_slug, id, page)
        .map_err(client_error_to_cli)?;
    print_json(&value, select)
}

fn run_update_comment(repository: &str, id: u64, comment_id: u64, content: &str, select: cli_fields::Select<'_>, identity: &Identity) -> Result<(), CliError> {
    let (workspace, repo_slug) = split_repository(repository)?;
    let body = build_comment_body(content, None, None);
    let value = authenticated_client(identity)?
        .update_pull_request_comment(workspace, repo_slug, id, comment_id, &body)
        .map_err(client_error_to_cli)?;
    // Exempt: a single comment object, fixed shape.
    print_json(&value, select.or_all())
}

/// Builds the `POST /2.0/repositories/{workspace}/{repo_slug}/pullrequests` request body.
/// `title` and `source` are always included; other fields only if set.
fn build_create_body(
    title: &str,
    source: &str,
    destination: Option<String>,
    description: Option<String>,
    close_source_branch: bool,
    reviewers: Vec<String>,
    draft: bool,
) -> Value {
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
    if !reviewers.is_empty() {
        let reviewer_objects: Vec<Value> = reviewers.into_iter().map(|uuid| json!({"uuid": uuid})).collect();
        map.insert("reviewers".to_string(), Value::Array(reviewer_objects));
    }
    if draft {
        map.insert("draft".to_string(), Value::Bool(true));
    }

    body
}

/// Turns `pr update`'s flags into a validated `PUT .../pullrequests/{id}` body: parses
/// reviewers, resolves draft status, and rejects an update with no fields set.
fn update_body_from_flags(
    title: Option<String>,
    description: Option<String>,
    destination: Option<String>,
    reviewers: Option<&str>,
    draft: bool,
    ready_for_review: bool,
) -> Result<Value, CliError> {
    let reviewer_uuids = split_reviewers(reviewers);
    let draft = draft_status(draft, ready_for_review);
    validate_update_has_field(title.as_deref(), description.as_deref(), destination.as_deref(), &reviewer_uuids, draft)?;
    Ok(build_update_body(title, description, destination, reviewer_uuids, draft))
}

/// Handles `PrCommand::Update` once the body is built and validated: calls
/// `PUT .../pullrequests/{id}` and prints the updated pull request.
fn run_update(repository: &str, id: u64, body: &Value, select: cli_fields::Select<'_>, identity: &Identity) -> Result<(), CliError> {
    let (workspace, repo_slug) = split_repository(repository)?;
    let value = authenticated_client(identity)?
        .update_pull_request(workspace, repo_slug, id, body)
        .map_err(client_error_to_cli)?;
    // Exempt: a single pull request object, fixed shape.
    print_json(&value, select.or_all())
}

/// Parses a comma-separated list of reviewer UUIDs (as accepted by `--reviewers` on
/// both `pr create` and `pr update`) into individual UUID strings.
fn split_reviewers(reviewers: Option<&str>) -> Vec<String> {
    reviewers
        .map(|s| s.split(',').map(str::trim).filter(|u| !u.is_empty()).map(str::to_string).collect())
        .unwrap_or_default()
}

/// Maps `pr update`'s mutually exclusive `--draft` / `--ready-for-review` flags to the
/// `draft` body field: `Some(true)`, `Some(false)`, or `None` (leave draft status as is).
/// clap's `conflicts_with` guarantees both are never set together.
fn draft_status(draft: bool, ready_for_review: bool) -> Option<bool> {
    if draft {
        Some(true)
    } else if ready_for_review {
        Some(false)
    } else {
        None
    }
}

/// Validates that at least one updatable field was provided for `pr update` — an
/// empty body would be a no-op `PUT` request.
fn validate_update_has_field(
    title: Option<&str>,
    description: Option<&str>,
    destination: Option<&str>,
    reviewers: &[String],
    draft: Option<bool>,
) -> Result<(), CliError> {
    if title.is_none() && description.is_none() && destination.is_none() && reviewers.is_empty() && draft.is_none() {
        return Err(CliError::InvalidInput {
            reason: "at least one of --title, --description, --destination, --reviewers, --draft, or --ready-for-review must be set".to_string(),
        });
    }
    Ok(())
}

/// Builds the `PUT /2.0/repositories/{workspace}/{repo_slug}/pullrequests/{id}` request
/// body. Only the fields passed as `Some`/non-empty are included, so the request only
/// changes what was asked. `reviewers`, if non-empty, replaces the entire reviewer list.
/// `draft`, if set, is sent as-is — `Some(false)` explicitly publishes a draft.
fn build_update_body(
    title: Option<String>,
    description: Option<String>,
    destination: Option<String>,
    reviewers: Vec<String>,
    draft: Option<bool>,
) -> Value {
    let mut body = json!({});
    let map = body.as_object_mut().unwrap_or_else(|| {
        unreachable!("body is always constructed as a JSON object literal above")
    });

    if let Some(title) = title {
        map.insert("title".to_string(), Value::String(title));
    }
    if let Some(description) = description {
        map.insert("description".to_string(), Value::String(description));
    }
    if let Some(destination) = destination {
        map.insert("destination".to_string(), json!({"branch": {"name": destination}}));
    }
    if !reviewers.is_empty() {
        let reviewer_objects: Vec<Value> = reviewers.into_iter().map(|uuid| json!({"uuid": uuid})).collect();
        map.insert("reviewers".to_string(), Value::Array(reviewer_objects));
    }
    if let Some(draft) = draft {
        map.insert("draft".to_string(), Value::Bool(draft));
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

/// Builds the comment request body for `POST .../pullrequests/{id}/comments` (create) and
/// `PUT .../pullrequests/{id}/comments/{comment_id}` (update, always with `inline = None`
/// and `parent = None`). `inline` adds an `inline` object with `path` and `to` (line number);
/// `parent` adds a `parent` object with the `id` of the comment being replied to.
fn build_comment_body(content: &str, inline: Option<(String, u64)>, parent: Option<u64>) -> Value {
    let mut body = json!({
        "content": {"raw": content},
    });
    let map = body.as_object_mut().unwrap_or_else(|| {
        unreachable!("body is always constructed as a JSON object literal above")
    });

    if let Some((path, line)) = inline {
        map.insert("inline".to_string(), json!({"path": path, "to": line}));
    }

    if let Some(parent_id) = parent {
        map.insert("parent".to_string(), json!({"id": parent_id}));
    }

    body
}

#[cfg(test)]
#[path = "../tests/commands/pr_tests.rs"]
mod tests;
