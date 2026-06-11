#![allow(clippy::unwrap_used, clippy::expect_used)]

use super::{AuthCommand, Cli, Command, RepoCommand};
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

#[test]
fn parses_repo_get() {
    let cli = Cli::try_parse_from(["bitbucket", "repo", "get", "lucabrognaracode/my-repo"]).expect("should parse");

    match cli.command {
        Command::Repo {
            command: RepoCommand::Get { repository },
        } => assert_eq!(repository, "lucabrognaracode/my-repo"),
        other => panic!("expected Repo Get, got {other:?}"),
    }
}
