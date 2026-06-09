mod auth;
mod cli;
mod client;
mod context;
mod error;
mod fields;

use std::process::ExitCode;

use clap::Parser;
use cli::{AuthCommand, Cli, Command, CommentCommand, IssueCommand};
use context::{authenticated_client, load_oauth_config, print_json};
use error::CliError;

fn run() -> Result<(), CliError> {
    let cli = Cli::parse();

    // Parse --select once; empty slice means "no filtering, print raw".
    let select_string = cli.select.unwrap_or_default();
    let select: Vec<&str> = if select_string.is_empty() {
        vec![]
    } else {
        select_string.split(',').map(str::trim).collect()
    };
    let select = select.as_slice();

    match cli.command {
        Command::Auth {
            command: AuthCommand::Login,
        } => {
            let oauth_config = load_oauth_config()?;
            let path = auth::credentials_path(&context::config_dir()?);
            let credentials = auth::login(&oauth_config).map_err(|e| CliError::LoginFailed {
                reason: e.to_string(),
            })?;
            auth::save_credentials(&path, &credentials).map_err(|e| {
                CliError::SaveCredentialsFailed {
                    path: path.display().to_string(),
                    reason: e.to_string(),
                }
            })?;
            println!("Logged in. Credentials saved to {}", path.display());
            Ok(())
        }

        Command::Auth {
            command: AuthCommand::Whoami,
        } => {
            let value = authenticated_client()?.get_myself().map_err(client_error_to_cli)?;
            print_json(&value, select)
        }

        Command::Issue { command } => {
            run_issue(command, select)
        }
    }
}

fn run_issue(command: IssueCommand, select: &[&str]) -> Result<(), CliError> {
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

fn client_error_to_cli(e: client::ClientError) -> CliError {
    match e {
        client::ClientError::Request(r) => CliError::ApiRequestFailed { reason: r },
        client::ClientError::Status { status, body } => CliError::ApiError { status, body },
    }
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("{err}");
            ExitCode::FAILURE
        }
    }
}
