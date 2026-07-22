#![allow(clippy::unwrap_used, clippy::expect_used)]

use super::{Cli, Command, UserCommand};
use clap::Parser;

#[test]
fn parses_init_with_no_flags() {
    let cli = Cli::try_parse_from(["atlassian-admin", "init"]).expect("should parse");

    assert!(matches!(cli.command, Command::Init { api_key: None, org_id: None }));
}

#[test]
fn parses_init_with_both_flags() {
    let cli = Cli::try_parse_from([
        "atlassian-admin",
        "init",
        "--api-key",
        "ATATT3xFfGF0",
        "--org-id",
        "abc123",
    ])
    .expect("should parse");

    match cli.command {
        Command::Init { api_key, org_id } => {
            assert_eq!(api_key.as_deref(), Some("ATATT3xFfGF0"));
            assert_eq!(org_id.as_deref(), Some("abc123"));
        }
        other => panic!("expected Command::Init, got {other:?}"),
    }
}

#[test]
fn parses_init_with_only_api_key() {
    let cli = Cli::try_parse_from(["atlassian-admin", "init", "--api-key", "key-only"])
        .expect("should parse");

    match cli.command {
        Command::Init { api_key, org_id } => {
            assert_eq!(api_key.as_deref(), Some("key-only"));
            assert_eq!(org_id, None);
        }
        other => panic!("expected Command::Init, got {other:?}"),
    }
}

#[test]
fn parses_doctor() {
    let cli = Cli::try_parse_from(["atlassian-admin", "doctor"]).expect("should parse");

    assert!(matches!(cli.command, Command::Doctor));
}

#[test]
fn parses_user_get() {
    let cli = Cli::try_parse_from([
        "atlassian-admin",
        "user",
        "get",
        "--account-id",
        "712020:b6d01943-f1de-4eb4-ab1a-300a17283d42",
    ])
    .expect("should parse");

    match cli.command {
        Command::User {
            command: UserCommand::Get { account_id },
        } => {
            assert_eq!(account_id, "712020:b6d01943-f1de-4eb4-ab1a-300a17283d42");
        }
        other => panic!("expected Command::User(Get), got {other:?}"),
    }
}

#[test]
fn rejects_user_get_without_account_id() {
    let result = Cli::try_parse_from(["atlassian-admin", "user", "get"]);

    assert!(result.is_err(), "--account-id should be required");
}

#[test]
fn parses_user_get_with_select() {
    let cli = Cli::try_parse_from([
        "atlassian-admin",
        "--select",
        "email,name",
        "user",
        "get",
        "--account-id",
        "some-id",
    ])
    .expect("should parse");

    assert_eq!(cli.select.as_deref(), Some("email,name"));
    assert!(matches!(
        cli.command,
        Command::User {
            command: UserCommand::Get { .. }
        }
    ));
}

#[test]
fn select_flag_is_none_and_select_all_false_when_absent() {
    let cli = Cli::try_parse_from(["atlassian-admin", "doctor"]).expect("should parse");

    assert!(cli.select.is_none());
    assert!(!cli.select_all);
}

#[test]
fn select_all_flag_parses() {
    let cli = Cli::try_parse_from(["atlassian-admin", "--select-all", "doctor"]).expect("should parse");

    assert!(cli.select_all);
    assert!(cli.select.is_none());
}

#[test]
fn select_and_select_all_together_are_rejected() {
    let result = Cli::try_parse_from([
        "atlassian-admin",
        "--select",
        "email",
        "--select-all",
        "doctor",
    ]);

    assert!(result.is_err(), "--select and --select-all should conflict");
}
