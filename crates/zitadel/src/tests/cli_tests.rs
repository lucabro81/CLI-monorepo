#![allow(clippy::unwrap_used, clippy::expect_used)]

use clap::Parser;
use clap::error::ErrorKind;

use super::{
    AuthCommand, AuthorizationState, Cli, Command, OrganizationCommand, ProjectCommand, UserCommand, UserState,
};

fn parse(args: &[&str]) -> Result<Cli, clap::Error> {
    Cli::try_parse_from(std::iter::once("zitadel").chain(args.iter().copied()))
}

fn user(cli: &Cli) -> Option<&str> {
    cli.user.as_ref().map(oauth_user_login::UserId::as_str)
}

fn error_kind(args: &[&str]) -> ErrorKind {
    parse(args).unwrap_err().kind()
}

#[test]
fn parses_auth_login() {
    let cli = parse(&["auth", "login"]).unwrap();

    assert_eq!(cli.user, None);
    assert!(matches!(
        cli.command,
        Command::Auth {
            command: AuthCommand::Login { remote: false, redirect_uri: None, code: None, state: None }
        }
    ));
}

#[test]
fn parses_auth_login_user() {
    let cli = parse(&["auth", "login", "--user", "alice"]).unwrap();

    assert_eq!(user(&cli), Some("alice"));
    assert!(matches!(
        cli.command,
        Command::Auth {
            command: AuthCommand::Login { remote: false, redirect_uri: None, code: None, state: None }
        }
    ));
}

#[test]
fn parses_auth_login_remote_start() {
    let cli = parse(&["auth", "login", "--user", "alice", "--remote", "--redirect-uri", "https://m.example/cb"]).unwrap();

    assert_eq!(user(&cli), Some("alice"));
    match cli.command {
        Command::Auth { command: AuthCommand::Login { remote, redirect_uri, code, state } } => {
            assert!(remote);
            assert_eq!(redirect_uri.as_deref(), Some("https://m.example/cb"));
            assert_eq!((code, state), (None, None));
        }
        other => panic!("got {other:?}"),
    }
}

#[test]
fn parses_auth_login_remote_complete() {
    let cli = parse(&["auth", "login", "--user", "alice", "--code", "c1", "--state", "s1"]).unwrap();

    assert_eq!(user(&cli), Some("alice"));
    match cli.command {
        Command::Auth { command: AuthCommand::Login { remote, redirect_uri, code, state } } => {
            assert!(!remote);
            assert_eq!(redirect_uri, None);
            assert_eq!((code.as_deref(), state.as_deref()), (Some("c1"), Some("s1")));
        }
        other => panic!("got {other:?}"),
    }
}

#[test]
fn remote_requires_redirect_uri() {
    // "--remote/--code need --user" is checked by LoginMode::from_flags, not clap:
    // clap cannot see a global --user written before the subcommand.
    assert_eq!(error_kind(&["auth", "login", "--user", "alice", "--remote"]), ErrorKind::MissingRequiredArgument);
}

#[test]
fn redirect_uri_requires_remote() {
    assert_eq!(
        error_kind(&["auth", "login", "--user", "alice", "--redirect-uri", "https://m.example/cb"]),
        ErrorKind::MissingRequiredArgument
    );
}

#[test]
fn code_requires_state_and_state_requires_code() {
    assert_eq!(error_kind(&["auth", "login", "--user", "alice", "--code", "c1"]), ErrorKind::MissingRequiredArgument);
    assert_eq!(error_kind(&["auth", "login", "--user", "alice", "--state", "s1"]), ErrorKind::MissingRequiredArgument);
}

#[test]
fn remote_start_and_completion_cannot_be_combined() {
    assert_eq!(
        error_kind(&[
            "auth", "login", "--user", "alice", "--remote", "--redirect-uri", "https://m.example/cb", "--code", "c1",
            "--state", "s1",
        ]),
        ErrorKind::ArgumentConflict
    );
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
        Command::Init { instance_url, key_file, client_id, user_app: false } => {
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
        Command::Init { instance_url: None, key_file: None, client_id: None, user_app: false }
    ));
}

#[test]
fn parses_user_search_with_defaults() {
    let cli = parse(&["user", "search"]).unwrap();

    match cli.command {
        Command::User { command: UserCommand::Search { email, email_exact, username, state, organization_id, limit, offset } } => {
            assert_eq!(email_exact, None);
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
        Command::User { command: UserCommand::Search { email, email_exact, username, state, organization_id, limit, offset } } => {
            assert_eq!(email_exact, None);
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

// ── user authorizations (issue #229) ──────────────────────────────────────

#[test]
fn parses_user_authorizations_with_defaults() {
    let cli = parse(&["user", "authorizations", "123"]).unwrap();

    match cli.command {
        Command::User { command: UserCommand::Authorizations { user_id, project_id, state, limit, offset } } => {
            assert_eq!(user_id, "123");
            assert_eq!((project_id, state, limit, offset), (None, None, 100, 0));
        }
        other => panic!("expected user authorizations, got {other:?}"),
    }
}

#[test]
fn parses_user_authorizations_with_all_flags() {
    let cli = parse(&[
        "user", "authorizations", "123", "--project-id", "456", "--state", "inactive", "--limit", "5", "--offset", "10",
    ])
    .unwrap();

    match cli.command {
        Command::User { command: UserCommand::Authorizations { user_id, project_id, state, limit, offset } } => {
            assert_eq!(user_id, "123");
            assert_eq!(project_id.as_deref(), Some("456"));
            assert_eq!(state, Some(AuthorizationState::Inactive));
            assert_eq!((limit, offset), (5, 10));
        }
        other => panic!("expected user authorizations, got {other:?}"),
    }
}

#[test]
fn user_authorizations_accepts_only_known_states() {
    // ZITADEL rejects unknown states with a 400; clap catches them first, naming the valid ones.
    assert_eq!(parse(&["user", "authorizations", "1", "--state", "active"]).map(|_| ()).ok(), Some(()));
    assert_eq!(error_kind(&["user", "authorizations", "1", "--state", "deleted"]), ErrorKind::InvalidValue);
}

#[test]
fn user_authorizations_requires_a_user_id_and_a_positive_limit() {
    assert_eq!(error_kind(&["user", "authorizations"]), ErrorKind::MissingRequiredArgument);
    assert_eq!(error_kind(&["user", "authorizations", "1", "--limit", "0"]), ErrorKind::ValueValidation);
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

// ── global --user <id> (issues #164, #175) ─────────────────────────────

#[test]
fn user_flag_defaults_to_the_service_user() {
    assert_eq!(parse(&["user", "search"]).unwrap().user, None);
}

#[test]
fn user_flag_is_accepted_after_any_subcommand() {
    for args in [
        &["user", "search", "--user", "alice"][..],
        &["user", "get", "123456789012345678", "--user", "alice"],
        &["organization", "list", "--user", "alice"],
        &["project", "list", "--user", "alice"],
        &["auth", "whoami", "--user", "alice"],
        &["doctor", "--user", "alice"],
        &["init", "--user", "alice"],
    ] {
        let cli = parse(args).unwrap_or_else(|e| panic!("{args:?}: {e}"));
        assert_eq!(user(&cli), Some("alice"), "{args:?} should select that person");
    }
}

#[test]
fn user_flag_is_accepted_before_the_subcommand() {
    let cli = parse(&["--user", "alice", "auth", "login", "--code", "c1", "--state", "s1"]).unwrap();

    assert_eq!(user(&cli), Some("alice"));
}

#[test]
fn user_flag_needs_the_persons_id() {
    // Issue #175: --user names the person; a bare --user would silently pick
    // whoever logged in last.
    assert_eq!(error_kind(&["user", "search", "--user"]), ErrorKind::InvalidValue);
}

#[test]
fn user_flag_accepts_an_id_with_a_colon() {
    // Issue #184: the agent's ids may be namespaced (`chat:u123`).
    let cli = parse(&["auth", "whoami", "--user", "chat:u123"]).unwrap();
    assert_eq!(cli.user.unwrap().as_str(), "chat:u123");
}

#[test]
fn user_flag_rejects_an_id_that_is_not_a_slug() {
    for id in ["Jane", "../etc", "a/b", ".hidden", "jane doe"] {
        let err = parse(&["auth", "whoami", "--user", id]).unwrap_err();
        assert_eq!(err.kind(), ErrorKind::ValueValidation, "{id}");
        assert!(err.to_string().contains("lowercase slug"), "{id}: {err}");
    }
}

#[test]
fn parses_auth_logout() {
    let cli = parse(&["auth", "logout", "--user", "alice"]).unwrap();

    assert_eq!(user(&cli), Some("alice"));
    assert!(matches!(cli.command, Command::Auth { command: AuthCommand::Logout }));
    assert_eq!(parse(&["auth", "logout"]).unwrap().user, None);
}

#[test]
fn parses_init_user_app() {
    // Issue #195: set up the Native app without logging anyone in.
    let cli = parse(&["init", "--user-app", "--client-id", "123@cli"]).unwrap();

    match cli.command {
        Command::Init { instance_url: None, key_file: None, client_id, user_app: true } => {
            assert_eq!(client_id.as_deref(), Some("123@cli"));
        }
        other => panic!("expected Init --user-app, got {other:?}"),
    }
}

// ── user search --email-exact and user idp-links (issue #230) ─────────────

#[test]
fn parses_user_search_email_exact() {
    let cli = parse(&["user", "search", "--email-exact", "jane@acme.com", "--select", "result.userId"]).unwrap();

    match cli.command {
        Command::User { command: UserCommand::Search { email, email_exact, .. } } => {
            assert_eq!(email, None);
            assert_eq!(email_exact.as_deref(), Some("jane@acme.com"));
        }
        other => panic!("expected user search, got {other:?}"),
    }
}

#[test]
fn user_search_email_and_email_exact_are_exclusive() {
    assert_eq!(
        error_kind(&["user", "search", "--email", "acme", "--email-exact", "jane@acme.com"]),
        ErrorKind::ArgumentConflict
    );
}

#[test]
fn parses_user_idp_links() {
    let cli = parse(&["user", "idp-links", "123"]).unwrap();
    assert!(matches!(
        cli.command,
        Command::User { command: UserCommand::IdpLinks { ref user_id, limit: 100, offset: 0 } } if user_id == "123"
    ));

    let cli = parse(&["user", "idp-links", "123", "--limit", "5", "--offset", "10"]).unwrap();
    assert!(matches!(cli.command, Command::User { command: UserCommand::IdpLinks { limit: 5, offset: 10, .. } }));
}

#[test]
fn user_idp_links_requires_a_user_id_and_a_positive_limit() {
    assert_eq!(error_kind(&["user", "idp-links"]), ErrorKind::MissingRequiredArgument);
    assert_eq!(error_kind(&["user", "idp-links", "1", "--limit", "0"]), ErrorKind::ValueValidation);
}

