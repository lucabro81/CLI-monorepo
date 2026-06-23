#![allow(clippy::unwrap_used, clippy::expect_used)]

use clap::Parser;

use super::{AuthCommand, Cli, Command, SpacesCommand};

#[test]
fn parses_auth_login_with_no_flags() {
    let cli = Cli::parse_from(["google-chat", "auth", "login"]);

    assert!(matches!(
        cli.command,
        Command::Auth {
            command: AuthCommand::Login { user: false }
        }
    ));
}

#[test]
fn parses_auth_login_with_user_flag() {
    let cli = Cli::parse_from(["google-chat", "auth", "login", "--user"]);

    assert!(matches!(
        cli.command,
        Command::Auth {
            command: AuthCommand::Login { user: true }
        }
    ));
}

#[test]
fn parses_init_with_no_flags() {
    let cli = Cli::parse_from(["google-chat", "init"]);

    assert!(matches!(
        cli.command,
        Command::Init {
            client_id: None,
            client_secret: None,
        }
    ));
}

#[test]
fn parses_init_with_client_id_and_secret() {
    let cli = Cli::parse_from([
        "google-chat",
        "init",
        "--client-id",
        "my-id",
        "--client-secret",
        "my-secret",
    ]);

    assert!(matches!(
        cli.command,
        Command::Init {
            client_id: Some(ref id),
            client_secret: Some(ref secret),
        } if id == "my-id" && secret == "my-secret"
    ));
}

#[test]
fn parses_doctor() {
    let cli = Cli::parse_from(["google-chat", "doctor"]);

    assert!(matches!(cli.command, Command::Doctor));
}

#[test]
fn doctor_with_select_flag() {
    let cli = Cli::parse_from([
        "google-chat",
        "doctor",
        "--select",
        "app_config.status,credentials.status",
    ]);

    assert!(matches!(cli.command, Command::Doctor));
    assert_eq!(
        cli.select,
        Some("app_config.status,credentials.status".to_string())
    );
}

#[test]
fn parses_spaces_list_with_no_optional_flags() {
    let cli = Cli::parse_from(["google-chat", "spaces", "list"]);

    assert!(matches!(
        cli.command,
        Command::Spaces {
            command: SpacesCommand::List {
                page_size: 100,
                page_token: None,
            }
        }
    ));
}

#[test]
fn parses_spaces_list_with_page_size_and_token() {
    let cli = Cli::parse_from([
        "google-chat",
        "spaces",
        "list",
        "--page-size",
        "20",
        "--page-token",
        "abc123",
    ]);

    assert!(matches!(
        cli.command,
        Command::Spaces {
            command: SpacesCommand::List {
                page_size: 20,
                page_token: Some(ref token),
            }
        } if token == "abc123"
    ));
}

#[test]
fn rejects_unknown_spaces_subcommand() {
    let result = Cli::try_parse_from(["google-chat", "spaces", "bogus"]);

    assert!(result.is_err());
}

#[test]
fn parses_global_select_flag_before_subcommand() {
    let cli = Cli::parse_from(["google-chat", "--select", "foo,bar", "auth", "login"]);

    assert_eq!(cli.select, Some("foo,bar".to_string()));
}

#[test]
fn parses_global_select_flag_after_subcommand() {
    let cli = Cli::parse_from(["google-chat", "auth", "login", "--select", "foo,bar"]);

    assert_eq!(cli.select, Some("foo,bar".to_string()));
}

#[test]
fn rejects_unknown_top_level_command() {
    let result = Cli::try_parse_from(["google-chat", "bogus"]);

    assert!(result.is_err());
}

#[test]
fn rejects_unknown_auth_subcommand() {
    let result = Cli::try_parse_from(["google-chat", "auth", "bogus"]);

    assert!(result.is_err());
}
