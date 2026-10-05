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
    assert!(parse(&["auth", "whoami", "--select", "user.id", "--select-all"]).is_err());
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
fn parses_doctor() {
    let cli = parse(&["doctor"]).unwrap();

    assert!(matches!(cli.command, Command::Doctor));
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
