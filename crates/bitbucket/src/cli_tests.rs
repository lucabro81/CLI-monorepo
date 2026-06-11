#![allow(clippy::unwrap_used, clippy::expect_used)]

use super::{AuthCommand, Cli, Command};
use clap::Parser;

#[test]
fn parses_auth_login() {
    let cli = Cli::try_parse_from(["bitbucket", "auth", "login"]).expect("should parse");

    assert!(matches!(
        cli.command,
        Command::Auth {
            command: AuthCommand::Login
        }
    ));
}

#[test]
fn parses_auth_whoami_with_select() {
    let cli = Cli::try_parse_from(["bitbucket", "--select", "uuid,display_name", "auth", "whoami"])
        .expect("should parse");

    assert_eq!(cli.select.as_deref(), Some("uuid,display_name"));
    assert!(matches!(
        cli.command,
        Command::Auth {
            command: AuthCommand::Whoami
        }
    ));
}
