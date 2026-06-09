mod auth;
mod cli;
mod client;
mod context;
mod error;

use std::process::ExitCode;

use clap::Parser;
use cli::{AuthCommand, Cli, Command, CommentCommand, IssueCommand};
use context::{authenticated_client, load_oauth_config, print_json};
use error::CliError;

fn run() -> Result<(), CliError> {
    let cli = Cli::parse();

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
            print_json(&value)
        }

        Command::Issue { command } => {
            let client = authenticated_client()?;
            match command {
                IssueCommand::Get { key } => {
                    let value = client.get_issue(&key).map_err(client_error_to_cli)?;
                    print_json(&value)
                }
                IssueCommand::Comment {
                    command: CommentCommand::Add { key, body },
                } => {
                    let value = client.add_comment(&key, &body).map_err(client_error_to_cli)?;
                    print_json(&value)
                }
                IssueCommand::Comment {
                    command: CommentCommand::Remove { key, id },
                } => {
                    client
                        .delete_comment(&key, &id)
                        .map_err(client_error_to_cli)?;
                    let result = serde_json::json!({"deleted": true, "id": id});
                    print_json(&result)
                }
            }
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
