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

use crate::client::{self, ClientError};
use crate::cli::{CommentCommand, IssueCommand};
use crate::context::{authenticated_client, print_json};
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
            command: CommentCommand::Add { key, body },
        } => {
            let client = authenticated_client()?;
            let value = client.add_comment(&key, &body).map_err(client_error_to_cli)?;
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

fn client_error_to_cli(e: ClientError) -> CliError {
    match e {
        client::ClientError::Request(r) => CliError::ApiRequestFailed { reason: r },
        client::ClientError::Status { status, body } => CliError::ApiError { status, body },
    }
}

#[cfg(test)]
#[path = "../tests/commands/issue_tests.rs"]
mod tests;
