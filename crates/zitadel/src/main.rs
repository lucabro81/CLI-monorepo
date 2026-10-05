//! Binary entry point for the `zitadel` CLI.
//!
//! Parses CLI arguments and dispatches to the command handlers in `commands/`.
//! `run()` returns `Result<(), CliError>`; `main()` prints any error to stderr
//! and maps it to a non-zero `ExitCode`. No `process::exit` anywhere.

mod auth;
mod cli;
mod commands;
mod context;
mod endpoints;
mod error;

use std::process::ExitCode;

use clap::Parser;
use cli::{AuthCommand, Cli, Command};
use error::CliError;

fn run() -> Result<(), CliError> {
    let cli = Cli::parse();

    match cli.command {
        Command::Auth {
            command: AuthCommand::Login,
        } => commands::auth::run_login(),
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
