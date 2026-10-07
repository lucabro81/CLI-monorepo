//! Binary entry point for the `zitadel` CLI.
//!
//! Parses CLI arguments and dispatches to the command handlers in `commands/`.
//! `run()` returns `Result<(), CliError>`; `main()` prints any error to stderr
//! and exits with `CliError::exit_code` (3 when the selected identity needs a
//! new login, 1 otherwise). No `process::exit` anywhere.

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

#[cfg(test)]
#[path = "tests/test_support.rs"]
mod test_support;

use std::process::ExitCode;

use clap::Parser;
use auth::Identity;
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

    // Global --user <id>: act as that person instead of the service user.
    let identity = Identity::from_user_flag(cli.user);

    match cli.command {
        Command::Init {
            instance_url,
            key_file,
            client_id,
            user_app: true,
        } => commands::init::run_init_user_app(
            &identity,
            instance_url.as_deref(),
            key_file.as_deref(),
            client_id.as_deref(),
            select,
        ),
        Command::Init {
            instance_url,
            key_file,
            client_id,
            user_app: false,
        } => commands::init::run_init(
            &identity,
            instance_url.as_deref(),
            key_file.as_deref(),
            client_id.as_deref(),
            select,
        ),
        Command::Doctor => {
            let (report, all_ok) = commands::doctor::run_doctor(&identity)?;
            // Exempt from mandatory --select: internally generated, small, fixed shape.
            context::print_json(&report, select.or_all())?;
            if !all_ok {
                return Err(CliError::DoctorCheckFailed);
            }
            Ok(())
        }
        Command::User { command } => commands::user::run(command, select, &identity),
        Command::Organization { command } => commands::organization::run(command, select, &identity),
        Command::Project { command } => commands::project::run(command, select, &identity),
        Command::Auth {
            command: AuthCommand::Login { remote: _, redirect_uri, code, state },
        } => commands::auth::run_login(
            commands::auth::LoginMode::from_flags(&identity, redirect_uri, code, state)?,
            select,
        ),
        Command::Auth {
            command: AuthCommand::Whoami,
        } => commands::auth::run_whoami(select, &identity),
        Command::Auth {
            command: AuthCommand::Logout,
        } => commands::auth::run_logout(select, &identity),
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
