//! Binary entry point for the `google-chat` CLI.
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

#[cfg(test)]
#[path = "tests/e2e_tests.rs"]
mod e2e_tests;

use std::process::ExitCode;

use clap::Parser;
use cli::{AuthCommand, Cli, Command};
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
            commands::init::run_init(client_id, client_secret)
        }
        Command::Doctor => {
            let (report, all_ok) = commands::doctor::run_doctor()?;
            context::print_json(&report, select)?;
            if !all_ok {
                return Err(CliError::DoctorCheckFailed);
            }
            Ok(())
        }
        Command::Auth {
            command: AuthCommand::Login { user },
        } => commands::auth::run_login(user),
        Command::Spaces { command } => commands::spaces::run(command, select),
        Command::Messages { command } => commands::messages::run(command, select),
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
