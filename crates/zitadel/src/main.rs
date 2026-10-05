//! Binary entry point for the `zitadel` CLI.
//!
//! Parses CLI arguments and dispatches to the command handlers in `commands/`.
//! `run()` returns `Result<(), CliError>`; `main()` prints any error to stderr
//! and maps it to a non-zero `ExitCode`. No `process::exit` anywhere.

mod auth;
mod cli;
mod client;
mod commands;
mod context;
mod endpoints;
mod error;

#[cfg(test)]
#[path = "tests/test_support.rs"]
mod test_support;

use std::process::ExitCode;

use clap::Parser;
use cli::{AuthCommand, Cli, Command};
use error::CliError;

fn run() -> Result<(), CliError> {
    let cli = Cli::parse();

    // Resolve --select/--select-all once; clap's conflicts_with guarantees they
    // are never both set.
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
        Command::Auth {
            command: AuthCommand::Login,
        } => commands::auth::run_login(),
        Command::Auth {
            command: AuthCommand::Whoami,
        } => commands::auth::run_whoami(select),
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
