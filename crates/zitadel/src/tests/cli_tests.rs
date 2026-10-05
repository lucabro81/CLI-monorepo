#![allow(clippy::unwrap_used, clippy::expect_used)]

use clap::Parser;
use clap::error::ErrorKind;

use super::{
    AuthCommand, Cli, Command, OrganizationCommand, ProjectCommand, UserCommand, UserState,
};

fn parse(args: &[&str]) -> Result<Cli, clap::Error> {
    Cli::try_parse_from(std::iter::once("zitadel").chain(args.iter().copied()))
}

fn error_kind(args: &[&str]) -> ErrorKind {
    parse(args).unwrap_err().kind()
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
fn parses_auth_whoami() {
    let cli = parse(&["auth", "whoami"]).unwrap();

    assert!(matches!(
        cli.command,
        Command::Auth { command: AuthCommand::Whoami }
    ));
    assert_eq!(cli.select, None);
    assert!(!cli.select_all);
}

#[test]
fn select_is_global_and_accepted_after_the_subcommand() {
    let cli = parse(&["auth", "whoami", "--select", "user.id,user.userName"]).unwrap();

    assert_eq!(cli.select.as_deref(), Some("user.id,user.userName"));
}

#[test]
fn select_all_is_global() {
    let cli = parse(&["--select-all", "auth", "whoami"]).unwrap();

    assert!(cli.select_all);
}

#[test]
fn select_and_select_all_conflict() {
    assert_eq!(error_kind(&["auth", "whoami", "--select", "user.id", "--select-all"]), ErrorKind::ArgumentConflict);
}

#[test]
fn parses_init_with_all_flags() {
    let cli = parse(&[
        "init", "--instance-url", "https://acme.zitadel.cloud", "--key-file", "/tmp/key.json",
        "--client-id", "123@cli",
    ])
    .unwrap();

    match cli.command {
        Command::Init { instance_url, key_file, client_id } => {
            assert_eq!(instance_url.as_deref(), Some("https://acme.zitadel.cloud"));
            assert_eq!(key_file, Some(std::path::PathBuf::from("/tmp/key.json")));
            assert_eq!(client_id.as_deref(), Some("123@cli"));
        }
        other => panic!("expected Init, got {other:?}"),
    }
}

#[test]
fn parses_init_with_no_flags() {
    let cli = parse(&["init"]).unwrap();

    assert!(matches!(
        cli.command,
        Command::Init { instance_url: None, key_file: None, client_id: None }
    ));
}

#[test]
fn parses_user_search_with_defaults() {
    let cli = parse(&["user", "search"]).unwrap();

    match cli.command {
        Command::User { command: UserCommand::Search { email, username, state, organization_id, limit, offset } } => {
            assert_eq!((email, username, state, organization_id), (None, None, None, None));
            assert_eq!((limit, offset), (100, 0));
        }
        other => panic!("expected user search, got {other:?}"),
    }
}

#[test]
fn parses_user_search_with_all_flags() {
    let cli = parse(&[
        "user", "search", "--email", "@acme.com", "--username", "john", "--state", "locked",
        "--organization-id", "org-1", "--limit", "10", "--offset", "20", "--select", "result.userId",
    ])
    .unwrap();

    match cli.command {
        Command::User { command: UserCommand::Search { email, username, state, organization_id, limit, offset } } => {
            assert_eq!(email.as_deref(), Some("@acme.com"));
            assert_eq!(username.as_deref(), Some("john"));
            assert_eq!(state, Some(UserState::Locked));
            assert_eq!(organization_id.as_deref(), Some("org-1"));
            assert_eq!((limit, offset), (10, 20));
        }
        other => panic!("expected user search, got {other:?}"),
    }
    assert_eq!(cli.select.as_deref(), Some("result.userId"));
}

#[test]
fn parses_user_get() {
    let cli = parse(&["user", "get", "123456789012345678"]).unwrap();

    assert!(matches!(
        cli.command,
        Command::User { command: UserCommand::Get { ref user_id } } if user_id == "123456789012345678"
    ));
}

#[test]
fn user_get_requires_a_user_id() {
    assert_eq!(error_kind(&["user", "get"]), ErrorKind::MissingRequiredArgument);
}

#[test]
fn user_search_rejects_unknown_state() {
    // ZITADEL silently returns zero results for an unknown state, so it must be
    // rejected client-side.
    assert_eq!(error_kind(&["user", "search", "--state", "suspended"]), ErrorKind::InvalidValue);
}

#[test]
fn user_search_rejects_zero_or_non_numeric_limit() {
    assert_eq!(error_kind(&["user", "search", "--limit", "0"]), ErrorKind::ValueValidation);
    assert_eq!(error_kind(&["user", "search", "--limit", "ten"]), ErrorKind::ValueValidation);
    assert_eq!(error_kind(&["user", "search", "--offset", "-1"]), ErrorKind::UnknownArgument);
}

#[test]
fn parses_organization_list_with_defaults() {
    let cli = parse(&["organization", "list"]).unwrap();

    assert!(matches!(
        cli.command,
        Command::Organization { command: OrganizationCommand::List { name: None, limit: 100, offset: 0 } }
    ));
}

#[test]
fn parses_organization_list_with_all_flags() {
    let cli = parse(&["organization", "list", "--name", "acme", "--limit", "5", "--offset", "10"]).unwrap();

    match cli.command {
        Command::Organization { command: OrganizationCommand::List { name, limit, offset } } => {
            assert_eq!(name.as_deref(), Some("acme"));
            assert_eq!((limit, offset), (5, 10));
        }
        other => panic!("expected organization list, got {other:?}"),
    }
}

#[test]
fn organization_list_rejects_zero_limit() {
    assert_eq!(error_kind(&["organization", "list", "--limit", "0"]), ErrorKind::ValueValidation);
}

#[test]
fn parses_project_list_with_defaults() {
    let cli = parse(&["project", "list"]).unwrap();

    assert!(matches!(
        cli.command,
        Command::Project {
            command: ProjectCommand::List { name: None, organization_id: None, limit: 100, offset: 0 }
        }
    ));
}

#[test]
fn parses_project_list_with_all_flags() {
    let cli = parse(&[
        "project", "list", "--name", "app", "--organization-id", "org-1", "--limit", "5", "--offset", "10",
    ])
    .unwrap();

    match cli.command {
        Command::Project { command: ProjectCommand::List { name, organization_id, limit, offset } } => {
            assert_eq!(name.as_deref(), Some("app"));
            assert_eq!(organization_id.as_deref(), Some("org-1"));
            assert_eq!((limit, offset), (5, 10));
        }
        other => panic!("expected project list, got {other:?}"),
    }
}

#[test]
fn project_list_rejects_zero_limit() {
    assert_eq!(error_kind(&["project", "list", "--limit", "0"]), ErrorKind::ValueValidation);
}

#[test]
fn parses_doctor() {
    let cli = parse(&["doctor"]).unwrap();

    assert!(matches!(cli.command, Command::Doctor));
}

#[test]
fn rejects_auth_without_subcommand() {
    assert_eq!(
        error_kind(&["auth"]),
        ErrorKind::DisplayHelpOnMissingArgumentOrSubcommand
    );
}

#[test]
fn rejects_unknown_auth_login_flag() {
    assert_eq!(error_kind(&["auth", "login", "--token", "x"]), ErrorKind::UnknownArgument);
}

#[test]
fn cli_definition_is_consistent() {
    use clap::CommandFactory;
    Cli::command().debug_assert();
}
