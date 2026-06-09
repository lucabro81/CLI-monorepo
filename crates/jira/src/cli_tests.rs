#![allow(clippy::unwrap_used, clippy::expect_used)]

use super::{AuthCommand, Cli, Command, IssueCommand};
use clap::Parser;

#[test]
fn parses_issue_get_with_key() {
    let cli = Cli::try_parse_from(["jira", "issue", "get", "PROJ-123"]).expect("should parse");

    match cli.command {
        Command::Issue {
            command: IssueCommand::Get { key },
        } => assert_eq!(key, "PROJ-123"),
        other => panic!("unexpected command: {other:?}"),
    }
}

#[test]
fn parses_auth_login() {
    let cli = Cli::try_parse_from(["jira", "auth", "login"]).expect("should parse");

    match cli.command {
        Command::Auth {
            command: AuthCommand::Login,
        } => {}
        other => panic!("unexpected command: {other:?}"),
    }
}

#[test]
fn parses_auth_whoami() {
    let cli = Cli::try_parse_from(["jira", "auth", "whoami"]).expect("should parse");

    match cli.command {
        Command::Auth {
            command: AuthCommand::Whoami,
        } => {}
        other => panic!("unexpected command: {other:?}"),
    }
}

#[test]
fn rejects_issue_get_without_key() {
    let result = Cli::try_parse_from(["jira", "issue", "get"]);

    assert!(result.is_err());
}

#[test]
fn rejects_unknown_command() {
    let result = Cli::try_parse_from(["jira", "bogus"]);

    assert!(result.is_err());
}
