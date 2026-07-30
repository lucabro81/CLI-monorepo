//! Handler for the `issue` command group and all its subcommands.
//!
//! Delegates all Jira API calls to `client::JiraClient`. Each subcommand
//! follows the same pattern: call the appropriate client method, map any
//! `ClientError` to `CliError`, then print the result as JSON (optionally
//! filtered via `--select`).
//!
//! Two subcommands contain non-trivial logic: `issue transition` fetches the
//! available transitions for an issue, matches the requested status name
//! case-insensitively, and fails with an actionable error listing valid
//! options if no match is found; `issue search` builds its `--stale-days`
//! JQL clause via `apply_stale_filter`.

use crate::cli::{CommentCommand, IssueCommand};
use crate::client::{ClientError, JiraClient};
use crate::context::{authenticated_client, client_error_to_cli, print_json};
use crate::error::CliError;

/// Dispatches an `IssueCommand` variant to the appropriate Jira API call.
///
/// `authenticated_client()` is called per-arm rather than once up front, so
/// that free, local validation (`Delete`'s `--confirm` check) runs before
/// the network round-trip a token refresh may require — a caller who forgot
/// `--confirm` finds out immediately instead of waiting on (and possibly
/// being confused by) an unrelated auth failure.
pub fn run(command: IssueCommand, select: cli_fields::Select<'_>) -> Result<(), CliError> {
    match command {
        IssueCommand::Search { jql, max_results, page_token, fields, stale_days } => {
            let client = authenticated_client()?;
            let jql = apply_stale_filter(&jql, stale_days);
            let value = client
                .search_issues(&jql, max_results, page_token.as_deref(), fields.as_deref())
                .map_err(client_error_to_cli)?;
            print_json(&value, select)
        }
        IssueCommand::Get { key } => {
            let client = authenticated_client()?;
            let value = client.get_issue(&key).map_err(client_error_to_cli)?;
            print_json(&value, select)
        }
        IssueCommand::Create {
            project,
            issue_type,
            summary,
            description,
            assignee,
            priority,
        } => {
            let client = authenticated_client()?;
            let value = client
                .create_issue(
                    &project,
                    &issue_type,
                    &summary,
                    description.as_deref(),
                    assignee.as_deref(),
                    priority.as_deref(),
                )
                .map_err(client_error_to_cli)?;
            // Exempt: POST /issue returns only {id, key, self} — small, fixed shape.
            print_json(&value, select.or_all())
        }
        IssueCommand::Delete {
            key,
            confirm,
            delete_subtasks,
        } => {
            if !confirm {
                return Err(CliError::DeleteNotConfirmed { key });
            }
            let client = authenticated_client()?;
            client
                .delete_issue(&key, delete_subtasks)
                .map_err(client_error_to_cli)?;
            let result = serde_json::json!({"deleted": true, "key": key});
            // Exempt: synthesized by us, always small.
            print_json(&result, select.or_all())
        }
        IssueCommand::Transitions { key } => {
            let client = authenticated_client()?;
            let value = client
                .list_transitions_json(&key)
                .map_err(client_error_to_cli)?;
            // Exempt: bounded workflow-transition list, no `expand` requested.
            print_json(&value, select.or_all())
        }
        IssueCommand::Transition { key, to } => {
            let client = authenticated_client()?;
            let transitions = client.get_transitions(&key).map_err(client_error_to_cli)?;
            let matched = transitions.iter().find(|t| t.name.eq_ignore_ascii_case(&to));
            let transition = matched.ok_or_else(|| {
                let available = transitions
                    .iter()
                    .map(|t| t.name.as_str())
                    .collect::<Vec<_>>()
                    .join(", ");
                CliError::TransitionNotFound {
                    name: to.clone(),
                    available,
                }
            })?;
            client
                .apply_transition(&key, &transition.id)
                .map_err(client_error_to_cli)?;
            let result =
                serde_json::json!({"transitioned": true, "key": key, "to": transition.name});
            // Exempt: synthesized by us, always small.
            print_json(&result, select.or_all())
        }
        IssueCommand::Comment {
            command: CommentCommand::Add { key, body, mention },
        } => {
            let client = authenticated_client()?;
            let content = build_comment_content(&client, &body, mention.as_deref())
                .map_err(client_error_to_cli)?;
            let value = client.add_comment(&key, &content).map_err(client_error_to_cli)?;
            // Exempt: a single comment object, fixed shape.
            print_json(&value, select.or_all())
        }
        IssueCommand::Comment {
            command: CommentCommand::Remove { key, id },
        } => {
            let client = authenticated_client()?;
            client
                .delete_comment(&key, &id)
                .map_err(client_error_to_cli)?;
            let result = serde_json::json!({"deleted": true, "id": id});
            // Exempt: synthesized by us, always small.
            print_json(&result, select.or_all())
        }
    }
}

/// Adds an `updated <= -Nd` condition to a JQL query, filtering to issues not
/// updated in at least `stale_days` days — JQL's own relative-date syntax,
/// evaluated server-side by Jira; no separate API call is needed to compute
/// staleness. `ORDER BY` must be the final clause in JQL, so the condition is
/// inserted right before it (case-insensitively) rather than appended blindly.
fn apply_stale_filter(jql: &str, stale_days: Option<u32>) -> String {
    let Some(days) = stale_days else {
        return jql.to_string();
    };
    let clause = format!("updated <= -{days}d");
    match jql.to_ascii_lowercase().find("order by") {
        Some(index) => format!("{} AND {} {}", jql[..index].trim_end(), clause, &jql[index..]),
        None => format!("{} AND {clause}", jql.trim_end()),
    }
}

/// A single piece of a comment `--body`: literal text, or a `{{mention:ACCOUNT_ID}}`
/// placeholder resolved to that account ID.
#[derive(Debug, PartialEq)]
enum BodySegment {
    Text(String),
    Mention(String),
}

/// Splits `body` on the `{{mention:ACCOUNT_ID}}` placeholder syntax into an ordered
/// sequence of text and mention segments. An unterminated placeholder (missing the
/// closing `}}`) is kept as literal text rather than silently dropped, since an
/// LLM-generated malformed placeholder shouldn't eat the rest of the comment.
fn parse_body_segments(body: &str) -> Vec<BodySegment> {
    const MARKER: &str = "{{mention:";
    let mut segments = Vec::new();
    let mut rest = body;

    while let Some(marker_start) = rest.find(MARKER) {
        let after_marker_start = &rest[marker_start + MARKER.len()..];
        let Some(end) = after_marker_start.find("}}") else {
            break;
        };
        let before = &rest[..marker_start];
        if !before.is_empty() {
            segments.push(BodySegment::Text(before.to_string()));
        }
        segments.push(BodySegment::Mention(after_marker_start[..end].to_string()));
        rest = &after_marker_start[end + 2..];
    }
    if !rest.is_empty() {
        segments.push(BodySegment::Text(rest.to_string()));
    }
    segments
}

/// Builds the ADF content nodes for `issue comment add`: an optional leading mention
/// node for `--mention`, followed by `body` parsed via [`parse_body_segments`] with
/// any `{{mention:ACCOUNT_ID}}` placeholders resolved to mention nodes in place.
fn build_comment_content(
    client: &JiraClient,
    body: &str,
    mention: Option<&str>,
) -> Result<Vec<serde_json::Value>, ClientError> {
    let mut content = Vec::new();

    if let Some(account_id) = mention {
        let display_name = mention_display_name(client, account_id)?;
        content.push(build_mention_node(account_id, &display_name));
        content.push(build_text_node(" "));
    }
    for segment in parse_body_segments(body) {
        match segment {
            BodySegment::Text(text) => content.push(build_text_node(&text)),
            BodySegment::Mention(account_id) => {
                let display_name = mention_display_name(client, &account_id)?;
                content.push(build_mention_node(&account_id, &display_name));
            }
        }
    }
    Ok(content)
}

/// Resolves `account_id` to its current display name via `GET /rest/api/3/user`, for
/// use as the fallback label on an ADF mention node. Falls back to the account ID
/// itself if the response is missing `displayName` (malformed-but-200 response) —
/// a genuine lookup failure (e.g. unknown account ID) still surfaces as a `ClientError`.
fn mention_display_name(client: &JiraClient, account_id: &str) -> Result<String, ClientError> {
    let user = client.get_user(account_id)?;
    Ok(user["displayName"].as_str().unwrap_or(account_id).to_string())
}

fn build_text_node(text: &str) -> serde_json::Value {
    serde_json::json!({"type": "text", "text": text})
}

fn build_mention_node(account_id: &str, display_name: &str) -> serde_json::Value {
    serde_json::json!({
        "type": "mention",
        "attrs": {"id": account_id, "text": format!("@{display_name}")}
    })
}

#[cfg(test)]
#[path = "../tests/commands/issue_tests.rs"]
mod tests;
