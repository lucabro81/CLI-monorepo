//! Binary entry point for the `confluence` CLI.
//!
//! Responsibilities are kept minimal: parse CLI arguments, resolve the
//! `--select` and `--user` flags once, then dispatch to the appropriate command handler in
//! `commands/`. All business logic lives in those modules.
//!
//! Error handling boundary: `run()` returns `Result<(), CliError>`; `main()`
//! prints any error to stderr and exits with `CliError::exit_code` (3 when the
//! selected identity needs a new login, 1 otherwise). No
//! `process::exit` is used anywhere in the codebase.

mod auth;
mod cli;
mod client;
mod commands;
mod context;
mod endpoints;
mod error;

use std::process::ExitCode;

use clap::Parser;
use auth::Identity;
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

    // Global --user <id>: act as that person instead of the service account.
    let identity = Identity::from_user_flag(cli.user);

    match cli.command {
        Command::Init { client_id, client_secret, user_app: false } => {
            commands::init::run_init(&identity, client_id, client_secret)
        }
        Command::Init { client_id, client_secret, user_app: true } => {
            commands::init::run_init_user_app(&identity, client_id, client_secret)
        }
        Command::Doctor => {
            let (report, all_ok) = commands::doctor::run_doctor(&identity)?;
            // Exempt from the mandatory --select requirement: the report is generated
            // internally (fixed, small shape, not an arbitrary external blob). An
            // explicit --select/--select-all is still honored if passed.
            context::print_json(&report, select.or_all())?;
            if !all_ok {
                return Err(CliError::DoctorCheckFailed);
            }
            Ok(())
        }
        Command::Auth { command: AuthCommand::Login { remote: _, redirect_uri, code, state } } => {
            commands::auth::run_login(commands::auth::LoginMode::from_flags(&identity, redirect_uri, code, state)?, select)
        }
        Command::Auth { command: AuthCommand::Whoami } => commands::auth::run_whoami(select, &identity),
        Command::Auth { command: AuthCommand::Logout } => commands::auth::run_logout(select, &identity),
        Command::Page { command } => commands::page::run(command, select, &identity),
        Command::Space { command } => commands::space::run(command, select, &identity),
        Command::Template { command } => commands::template::run(command, select, &identity),
    }
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("{err}");
            ExitCode::from(err.exit_code())
        }
    }
}
