//! Handler for the `issue` command group and all its subcommands.
//!
//! Delegates all Jira API calls to `client::JiraClient`. Each subcommand
//! follows the same pattern: call the appropriate client method, map any
//! `ClientError` to `CliError`, then print the result as JSON (optionally
//! filtered via `--select`).
//!
//! The `issue transition` subcommand contains the only non-trivial logic:
//! it fetches the available transitions for an issue, matches the requested
//! status name case-insensitively, and fails with an actionable error listing
//! valid options if no match is found.

use crate::client::{self, ClientError};
use crate::cli::{CommentCommand, IssueCommand};
use crate::context::{authenticated_client, print_json};
use crate::error::CliError;

/// Dispatches an `IssueCommand` variant to the appropriate Jira API call.
pub fn run(command: IssueCommand, select: &[&str]) -> Result<(), CliError> {
    let client = authenticated_client()?;
    match command {
        IssueCommand::Search { jql, max_results, page_token, fields } => {
            let value = client
                .search_issues(&jql, max_results, page_token.as_deref(), fields.as_deref())
                .map_err(client_error_to_cli)?;
            print_json(&value, select)
        }
        IssueCommand::Get { key } => {
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
            print_json(&value, select)
        }
        IssueCommand::Delete {
            key,
            confirm,
            delete_subtasks,
        } => {
            if !confirm {
                return Err(CliError::DeleteNotConfirmed { key });
            }
            client
                .delete_issue(&key, delete_subtasks)
                .map_err(client_error_to_cli)?;
            let result = serde_json::json!({"deleted": true, "key": key});
            print_json(&result, select)
        }
        IssueCommand::Transitions { key } => {
            let value = client
                .list_transitions_json(&key)
                .map_err(client_error_to_cli)?;
            print_json(&value, select)
        }
        IssueCommand::Transition { key, to } => {
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
            print_json(&result, select)
        }
        IssueCommand::Comment {
            command: CommentCommand::Add { key, body },
        } => {
            let value = client.add_comment(&key, &body).map_err(client_error_to_cli)?;
            print_json(&value, select)
        }
        IssueCommand::Comment {
            command: CommentCommand::Remove { key, id },
        } => {
            client
                .delete_comment(&key, &id)
                .map_err(client_error_to_cli)?;
            let result = serde_json::json!({"deleted": true, "id": id});
            print_json(&result, select)
        }
    }
}

fn client_error_to_cli(e: ClientError) -> CliError {
    match e {
        client::ClientError::Request(r) => CliError::ApiRequestFailed { reason: r },
        client::ClientError::Status { status, body } => CliError::ApiError { status, body },
    }
}
