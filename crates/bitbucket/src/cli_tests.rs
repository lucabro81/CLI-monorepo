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
fn parses_init_with_flags() {
    let cli = Cli::try_parse_from(["bitbucket", "init", "--client-id", "abc", "--client-secret", "xyz"])
        .expect("should parse");

    match cli.command {
        Command::Init { client_id, client_secret } => {
            assert_eq!(client_id.as_deref(), Some("abc"));
            assert_eq!(client_secret.as_deref(), Some("xyz"));
        }
        other => panic!("expected Init, got {other:?}"),
    }
}

#[test]
fn parses_init_without_flags() {
    let cli = Cli::try_parse_from(["bitbucket", "init"]).expect("should parse");

    match cli.command {
        Command::Init { client_id, client_secret } => {
            assert_eq!(client_id, None);
            assert_eq!(client_secret, None);
        }
        other => panic!("expected Init, got {other:?}"),
    }
}

#[test]
fn parses_doctor() {
    let cli = Cli::try_parse_from(["bitbucket", "doctor"]).expect("should parse");

    assert!(matches!(cli.command, Command::Doctor));
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

#[test]
fn parses_repo_list_without_page() {
    let cli = Cli::try_parse_from(["bitbucket", "repo", "list", "lucabrognaracode"]).expect("should parse");

    match cli.command {
        Command::Repo {
            command: RepoCommand::List { workspace, page },
        } => {
            assert_eq!(workspace, "lucabrognaracode");
            assert_eq!(page, None);
        }
        other => panic!("expected Repo List, got {other:?}"),
    }
}

#[test]
fn parses_repo_list_with_page() {
    let cli = Cli::try_parse_from(["bitbucket", "repo", "list", "lucabrognaracode", "--page", "2"]).expect("should parse");

    match cli.command {
        Command::Repo {
            command: RepoCommand::List { workspace, page },
        } => {
            assert_eq!(workspace, "lucabrognaracode");
            assert_eq!(page, Some(2));
        }
        other => panic!("expected Repo List, got {other:?}"),
    }
}
