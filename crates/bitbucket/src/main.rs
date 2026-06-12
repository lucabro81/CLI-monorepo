//! Binary entry point for the `bitbucket` CLI.
//!
//! Responsibilities are kept minimal: parse CLI arguments, resolve the
//! `--select` flag once, then dispatch to the appropriate command handler in
//! `commands/`. All business logic lives in those modules.
//!
//! Error handling boundary: `run()` returns `Result<(), CliError>`; `main()`
//! prints any error to stderr and maps it to a non-zero `ExitCode`. No
//! `process::exit` is used anywhere in the codebase.

mod auth;
mod cli;
mod client;
mod commands;
mod context;
mod endpoints;
mod error;
mod fields;

use std::process::ExitCode;

use clap::Parser;
use cli::{AuthCommand, Cli, Command};
use context::print_json;
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
        Command::Init { client_id, client_secret } => commands::init::run_init(client_id, client_secret),
        Command::Doctor => {
            let (report, all_ok) = commands::doctor::run_doctor()?;
            print_json(&report, select)?;
            if !all_ok {
                return Err(CliError::DoctorCheckFailed);
            }
            Ok(())
        }
        Command::Auth { command: AuthCommand::Login } => commands::auth::run_login(),
        Command::Auth { command: AuthCommand::Whoami } => commands::auth::run_whoami(select),
        Command::Repo { command } => commands::repo::run(command, select),
        Command::Pr { command } => commands::pr::run(command, select),
        Command::Branch { command } => commands::branch::run(command, select),
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
