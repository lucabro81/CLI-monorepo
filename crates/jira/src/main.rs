mod auth;
mod cli;
mod client;
mod context;
mod doctor;
mod error;
mod fields;
mod init;
mod issue;

use std::process::ExitCode;

use clap::Parser;
use cli::{AuthCommand, Cli, Command};
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
        Command::Init { client_id, client_secret } => {
            init::run_init(client_id, client_secret)
        }
        Command::Doctor => {
            let (report, all_ok) = doctor::run_doctor()?;
            print_json(&report, select)?;
            if !all_ok {
                return Err(CliError::DoctorCheckFailed);
            }
            Ok(())
        }
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
            let value = authenticated_client()?.get_myself().map_err(|e| {
                CliError::ApiRequestFailed { reason: e.to_string() }
            })?;
            print_json(&value, select)
        }

        Command::Issue { command } => issue::run(command, select),
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
