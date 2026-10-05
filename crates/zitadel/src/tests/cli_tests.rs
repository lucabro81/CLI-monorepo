#![allow(clippy::unwrap_used, clippy::expect_used)]

use clap::Parser;

use super::{AuthCommand, Cli, Command};

fn parse(args: &[&str]) -> Result<Cli, clap::Error> {
    Cli::try_parse_from(std::iter::once("zitadel").chain(args.iter().copied()))
}

#[test]
fn parses_auth_login() {
    let cli = parse(&["auth", "login"]).unwrap();

    assert!(matches!(
        cli.command,
        Command::Auth { command: AuthCommand::Login }
    ));
}

#[test]
fn rejects_auth_without_subcommand() {
    assert!(parse(&["auth"]).is_err());
}

#[test]
fn rejects_unknown_auth_login_flag() {
    assert!(parse(&["auth", "login", "--token", "x"]).is_err());
}

#[test]
fn cli_definition_is_consistent() {
    use clap::CommandFactory;
    Cli::command().debug_assert();
}
