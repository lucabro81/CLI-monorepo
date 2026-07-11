//! Binary entry point for the `jira` CLI.
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

#[cfg(test)]
#[path = "tests/e2e_tests.rs"]
mod e2e_tests;

use std::process::ExitCode;

use clap::Parser;
use cli::{AuthCommand, Cli, Command};
use error::CliError;

fn run() -> Result<(), CliError> {
    let cli = Cli::parse();

    // Resolve --select/--select-all once into a single Select value; clap's
    // conflicts_with guarantees they are never both set.
    let select_string = cli.select.unwrap_or_default();
    let select_paths: Vec<&str> = if select_string.is_empty() {
        vec![]
    } else {
        select_string.split(',').map(str::trim).collect()
    };
    let select = if cli.select_all {
        cli_fields::Select::All
    } else if select_paths.is_empty() {
        cli_fields::Select::Required
    } else {
        cli_fields::Select::Fields(&select_paths)
    };

    match cli.command {
        Command::Init { client_id, client_secret } => {
            commands::init::run_init(client_id, client_secret)
        }
        Command::Doctor => {
            let (report, all_ok) = commands::doctor::run_doctor()?;
            // Exempt from the mandatory --select requirement: the report is generated
            // internally (fixed, small shape, not an arbitrary external blob). An
            // explicit --select/--select-all is still honored if passed.
            context::print_json(&report, select.or_all())?;
            if !all_ok {
                return Err(CliError::DoctorCheckFailed);
            }
            Ok(())
        }
        Command::Auth { command: AuthCommand::Login { user } } => commands::auth::run_login(user),
        Command::Auth { command: AuthCommand::Whoami } => commands::auth::run_whoami(select),
        Command::Issue { command } => commands::issue::run(command, select),
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
