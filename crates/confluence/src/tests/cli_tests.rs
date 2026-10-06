#![allow(clippy::unwrap_used, clippy::expect_used)]

use clap::Parser;

use super::{AuthCommand, Cli, Command, PageCommand, SpaceCommand, TemplateCommand};

fn user(cli: &Cli) -> Option<&str> {
    cli.user.as_ref().map(oauth_user_login::UserId::as_str)
}

#[test]
fn parses_auth_login() {
    let cli = Cli::try_parse_from(["confluence", "auth", "login"]).expect("should parse");

    assert_eq!(cli.user, None, "default should be service account (client_credentials)");
    match cli.command {
        Command::Auth {
            command: AuthCommand::Login { remote, redirect_uri, code, state },
        } => {
            assert!(!remote);
            assert_eq!((redirect_uri, code, state), (None, None, None));
        }
        other => panic!("unexpected command: {other:?}"),
    }
}

#[test]
fn parses_auth_login_with_user_flag() {
    let cli = Cli::try_parse_from(["confluence", "auth", "login", "--user", "alice"]).expect("should parse");

    assert_eq!(user(&cli), Some("alice"), "--user should select the interactive 3LO flow");
    match cli.command {
        Command::Auth {
            command: AuthCommand::Login { remote, .. },
        } => assert!(!remote),
        other => panic!("unexpected command: {other:?}"),
    }
}

fn login_error_kind(args: &[&str]) -> clap::error::ErrorKind {
    Cli::try_parse_from(["confluence", "auth", "login"].iter().chain(args)).unwrap_err().kind()
}

#[test]
fn parses_auth_login_remote_start() {
    let cli = Cli::try_parse_from([
        "confluence", "auth", "login", "--user", "alice", "--remote", "--redirect-uri", "https://m.example/cb",
    ])
    .expect("should parse");

    assert_eq!(user(&cli), Some("alice"));
    match cli.command {
        Command::Auth { command: AuthCommand::Login { remote, redirect_uri, code, state } } => {
            assert!(remote);
            assert_eq!(redirect_uri.as_deref(), Some("https://m.example/cb"));
            assert_eq!((code, state), (None, None));
        }
        other => panic!("unexpected command: {other:?}"),
    }
}

#[test]
fn parses_auth_login_remote_complete() {
    let cli = Cli::try_parse_from(["confluence", "auth", "login", "--user", "alice", "--code", "c1", "--state", "s1"])
        .expect("should parse");

    assert_eq!(user(&cli), Some("alice"));
    match cli.command {
        Command::Auth { command: AuthCommand::Login { remote, redirect_uri, code, state } } => {
            assert!(!remote);
            assert_eq!(redirect_uri, None);
            assert_eq!((code.as_deref(), state.as_deref()), (Some("c1"), Some("s1")));
        }
        other => panic!("unexpected command: {other:?}"),
    }
}

#[test]
fn auth_login_remote_flag_combinations_are_enforced() {
    use clap::error::ErrorKind::{ArgumentConflict, MissingRequiredArgument};
    // "--remote/--code need --user" is checked by LoginMode::from_flags, not clap:
    // clap cannot see a global --user written before the subcommand.
    assert_eq!(login_error_kind(&["--user", "alice", "--remote"]), MissingRequiredArgument);
    assert_eq!(login_error_kind(&["--user", "alice", "--redirect-uri", "https://m/cb"]), MissingRequiredArgument);
    assert_eq!(login_error_kind(&["--user", "alice", "--code", "c"]), MissingRequiredArgument);
    assert_eq!(login_error_kind(&["--user", "alice", "--state", "s"]), MissingRequiredArgument);
    assert_eq!(
        login_error_kind(&["--user", "alice", "--remote", "--redirect-uri", "https://m/cb", "--code", "c", "--state", "s"]),
        ArgumentConflict
    );
}

#[test]
fn parses_auth_whoami() {
    let cli = Cli::try_parse_from(["confluence", "auth", "whoami"]).expect("should parse");

    match cli.command {
        Command::Auth {
            command: AuthCommand::Whoami,
        } => {}
        other => panic!("unexpected command: {other:?}"),
    }
}

// --- global --user <id> (issues #164, #175) ---

#[test]
fn user_flag_defaults_to_the_service_identity() {
    let cli = Cli::try_parse_from(["confluence", "page", "get", "123456"]).expect("should parse");

    assert_eq!(cli.user, None);
}

#[test]
fn user_flag_is_accepted_after_any_subcommand() {
    for args in [
        &["confluence", "page", "get", "123456", "--user", "alice"][..],
        &["confluence", "space", "list", "--user", "alice"],
        &["confluence", "template", "list", "--user", "alice"],
        &["confluence", "auth", "whoami", "--user", "alice"],
        &["confluence", "doctor", "--user", "alice"],
        &["confluence", "init", "--user", "alice"],
    ] {
        let cli = Cli::try_parse_from(args).unwrap_or_else(|e| panic!("{args:?}: {e}"));
        assert_eq!(user(&cli), Some("alice"), "{args:?} should select that person");
    }
}

#[test]
fn user_flag_is_accepted_before_the_subcommand() {
    let cli = Cli::try_parse_from(["confluence", "--user", "alice", "page", "get", "123456"]).expect("should parse");

    assert_eq!(user(&cli), Some("alice"));
}

#[test]
fn remote_login_flags_accept_a_user_flag_placed_before_the_subcommand() {
    // --remote/--code work with the global --user <id> wherever it is written.
    let cli = Cli::try_parse_from(["confluence", "--user", "alice", "auth", "login", "--code", "c", "--state", "s"])
        .expect("should parse");

    assert_eq!(user(&cli), Some("alice"));
}

#[test]
fn user_flag_needs_the_persons_id() {
    // Issue #175: --user names the person; a bare --user would silently pick
    // whoever logged in last.
    let err = Cli::try_parse_from(["confluence", "page", "get", "123456", "--user"]).unwrap_err();
    assert_eq!(err.kind(), clap::error::ErrorKind::InvalidValue);
}

#[test]
fn user_flag_rejects_an_id_that_is_not_a_slug() {
    for id in ["Jane", "../etc", "a/b", ".hidden", "jane doe"] {
        let err = Cli::try_parse_from(["confluence", "auth", "whoami", "--user", id]).unwrap_err();
        assert_eq!(err.kind(), clap::error::ErrorKind::ValueValidation, "{id}");
        assert!(err.to_string().contains("lowercase slug"), "{id}: {err}");
    }
}

#[test]
fn parses_auth_logout() {
    let cli = Cli::try_parse_from(["confluence", "auth", "logout", "--user", "alice"]).expect("should parse");

    assert_eq!(user(&cli), Some("alice"));
    assert!(matches!(cli.command, Command::Auth { command: AuthCommand::Logout }));
    let service = Cli::try_parse_from(["confluence", "auth", "logout"]).expect("should parse");
    assert_eq!(service.user, None);
}

#[test]
fn rejects_unknown_command() {
    let result = Cli::try_parse_from(["confluence", "bogus"]);

    assert!(result.is_err());
}

#[test]
fn parses_init_no_flags() {
    let cli = Cli::try_parse_from(["confluence", "init"]).expect("should parse");

    match cli.command {
        Command::Init { client_id, client_secret } => {
            assert!(client_id.is_none());
            assert!(client_secret.is_none());
        }
        other => panic!("unexpected: {other:?}"),
    }
}

#[test]
fn parses_init_with_both_flags() {
    let cli = Cli::try_parse_from([
        "confluence", "init", "--client-id", "abc123", "--client-secret", "s3cr3t",
    ])
    .expect("should parse");

    match cli.command {
        Command::Init { client_id, client_secret } => {
            assert_eq!(client_id.as_deref(), Some("abc123"));
            assert_eq!(client_secret.as_deref(), Some("s3cr3t"));
        }
        other => panic!("unexpected: {other:?}"),
    }
}

#[test]
fn parses_init_with_only_client_id() {
    // Partial flags are allowed at parse time; runtime will prompt for missing value.
    let cli = Cli::try_parse_from(["confluence", "init", "--client-id", "abc123"])
        .expect("should parse");

    match cli.command {
        Command::Init { client_id, client_secret } => {
            assert_eq!(client_id.as_deref(), Some("abc123"));
            assert!(client_secret.is_none());
        }
        other => panic!("unexpected: {other:?}"),
    }
}

#[test]
fn parses_init_with_only_client_secret() {
    let cli = Cli::try_parse_from(["confluence", "init", "--client-secret", "s3cr3t"])
        .expect("should parse");

    match cli.command {
        Command::Init { client_id, client_secret } => {
            assert!(client_id.is_none());
            assert_eq!(client_secret.as_deref(), Some("s3cr3t"));
        }
        other => panic!("unexpected: {other:?}"),
    }
}

#[test]
fn parses_doctor() {
    let cli = Cli::try_parse_from(["confluence", "doctor"]).expect("should parse");

    assert!(matches!(cli.command, Command::Doctor));
}

#[test]
fn doctor_with_select_flag() {
    let cli = Cli::try_parse_from([
        "confluence", "doctor", "--select", "app_config.status,credentials.status",
    ])
    .expect("should parse");

    assert!(matches!(cli.command, Command::Doctor));
    assert_eq!(
        cli.select.as_deref(),
        Some("app_config.status,credentials.status")
    );
}

#[test]
fn select_all_flag_parses() {
    let cli = Cli::try_parse_from(["confluence", "--select-all", "auth", "whoami"])
        .expect("should parse");

    assert!(cli.select_all);
    assert!(cli.select.is_none());
}

#[test]
fn select_and_select_all_together_are_rejected() {
    let result = Cli::try_parse_from([
        "confluence",
        "--select",
        "displayName",
        "--select-all",
        "auth",
        "whoami",
    ]);

    assert!(result.is_err(), "--select and --select-all should conflict");
}

// --- page get ---

#[test]
fn parses_page_get_with_id() {
    let cli = Cli::try_parse_from(["confluence", "page", "get", "123456"]).expect("should parse");

    match cli.command {
        Command::Page {
            command: PageCommand::Get { id },
        } => assert_eq!(id, "123456"),
        other => panic!("unexpected command: {other:?}"),
    }
}

#[test]
fn rejects_page_get_without_id() {
    let result = Cli::try_parse_from(["confluence", "page", "get"]);

    assert!(result.is_err());
}

// --- page create ---

#[test]
fn parses_page_create_with_body() {
    let cli = Cli::try_parse_from([
        "confluence", "page", "create", "--space-id", "98765", "--title", "Sprint Notes",
        "--body", "<p>hi</p>",
    ])
    .expect("should parse");

    match cli.command {
        Command::Page {
            command:
                PageCommand::Create {
                    space_id,
                    title,
                    parent_id,
                    body,
                    body_file,
                    template_id,
                },
        } => {
            assert_eq!(space_id, "98765");
            assert_eq!(title, "Sprint Notes");
            assert!(parent_id.is_none());
            assert_eq!(body.as_deref(), Some("<p>hi</p>"));
            assert!(body_file.is_none());
            assert!(template_id.is_none());
        }
        other => panic!("unexpected command: {other:?}"),
    }
}

#[test]
fn parses_page_create_with_body_file() {
    let cli = Cli::try_parse_from([
        "confluence", "page", "create", "--space-id", "98765", "--title", "Runbook",
        "--body-file", "./runbook.html",
    ])
    .expect("should parse");

    match cli.command {
        Command::Page {
            command: PageCommand::Create { body_file, body, template_id, .. },
        } => {
            assert_eq!(body_file.as_deref(), Some("./runbook.html"));
            assert!(body.is_none());
            assert!(template_id.is_none());
        }
        other => panic!("unexpected command: {other:?}"),
    }
}

#[test]
fn parses_page_create_with_template_id_and_parent_id() {
    let cli = Cli::try_parse_from([
        "confluence", "page", "create", "--space-id", "98765", "--title", "Retro",
        "--template-id", "4321", "--parent-id", "111222",
    ])
    .expect("should parse");

    match cli.command {
        Command::Page {
            command: PageCommand::Create { template_id, parent_id, .. },
        } => {
            assert_eq!(template_id.as_deref(), Some("4321"));
            assert_eq!(parent_id.as_deref(), Some("111222"));
        }
        other => panic!("unexpected command: {other:?}"),
    }
}

#[test]
fn rejects_page_create_with_body_and_body_file_together() {
    let result = Cli::try_parse_from([
        "confluence", "page", "create", "--space-id", "98765", "--title", "X",
        "--body", "<p>hi</p>", "--body-file", "./t.html",
    ]);

    assert!(result.is_err(), "--body and --body-file should conflict");
}

#[test]
fn rejects_page_create_with_body_file_and_template_id_together() {
    let result = Cli::try_parse_from([
        "confluence", "page", "create", "--space-id", "98765", "--title", "X",
        "--body-file", "./t.html", "--template-id", "4321",
    ]);

    assert!(result.is_err(), "--body-file and --template-id should conflict");
}

#[test]
fn rejects_page_create_missing_space_id() {
    let result = Cli::try_parse_from([
        "confluence", "page", "create", "--title", "X", "--body", "<p>hi</p>",
    ]);

    assert!(result.is_err());
}

#[test]
fn rejects_page_create_missing_title() {
    let result = Cli::try_parse_from([
        "confluence", "page", "create", "--space-id", "98765", "--body", "<p>hi</p>",
    ]);

    assert!(result.is_err());
}

// --- page update ---

#[test]
fn parses_page_update_with_title_only() {
    let cli = Cli::try_parse_from(["confluence", "page", "update", "123456", "--title", "New"])
        .expect("should parse");

    match cli.command {
        Command::Page {
            command: PageCommand::Update { id, title, body },
        } => {
            assert_eq!(id, "123456");
            assert_eq!(title.as_deref(), Some("New"));
            assert!(body.is_none());
        }
        other => panic!("unexpected command: {other:?}"),
    }
}

#[test]
fn parses_page_update_with_body_only() {
    let cli = Cli::try_parse_from([
        "confluence", "page", "update", "123456", "--body", "<p>new</p>",
    ])
    .expect("should parse");

    match cli.command {
        Command::Page {
            command: PageCommand::Update { title, body, .. },
        } => {
            assert!(title.is_none());
            assert_eq!(body.as_deref(), Some("<p>new</p>"));
        }
        other => panic!("unexpected command: {other:?}"),
    }
}

#[test]
fn parses_page_update_with_neither_flag() {
    // Parsing allows this; run_update's validate_update_target rejects it at runtime.
    let cli = Cli::try_parse_from(["confluence", "page", "update", "123456"]).expect("should parse");

    match cli.command {
        Command::Page {
            command: PageCommand::Update { title, body, .. },
        } => {
            assert!(title.is_none());
            assert!(body.is_none());
        }
        other => panic!("unexpected command: {other:?}"),
    }
}

#[test]
fn rejects_page_update_without_id() {
    let result = Cli::try_parse_from(["confluence", "page", "update"]);

    assert!(result.is_err());
}

// --- page delete ---

#[test]
fn parses_page_delete_with_confirm() {
    let cli = Cli::try_parse_from(["confluence", "page", "delete", "123456", "--confirm"])
        .expect("should parse");

    match cli.command {
        Command::Page {
            command: PageCommand::Delete { id, confirm, purge },
        } => {
            assert_eq!(id, "123456");
            assert!(confirm);
            assert!(!purge, "default should not purge");
        }
        other => panic!("unexpected command: {other:?}"),
    }
}

#[test]
fn parses_page_delete_without_confirm_defaults_false() {
    let cli =
        Cli::try_parse_from(["confluence", "page", "delete", "123456"]).expect("should parse");

    match cli.command {
        Command::Page {
            command: PageCommand::Delete { confirm, .. },
        } => assert!(!confirm),
        other => panic!("unexpected command: {other:?}"),
    }
}

#[test]
fn parses_page_delete_with_purge() {
    let cli = Cli::try_parse_from([
        "confluence", "page", "delete", "123456", "--confirm", "--purge",
    ])
    .expect("should parse");

    match cli.command {
        Command::Page {
            command: PageCommand::Delete { purge, .. },
        } => assert!(purge),
        other => panic!("unexpected command: {other:?}"),
    }
}

#[test]
fn rejects_page_delete_without_id() {
    let result = Cli::try_parse_from(["confluence", "page", "delete"]);

    assert!(result.is_err());
}

// --- page search ---

#[test]
fn parses_page_search_with_cql() {
    let cli = Cli::try_parse_from([
        "confluence", "page", "search", "--cql", "type=page AND space=ENG",
    ])
    .expect("should parse");

    match cli.command {
        Command::Page {
            command: PageCommand::Search { cql, limit, start },
        } => {
            assert_eq!(cql, "type=page AND space=ENG");
            assert_eq!(limit, 25);
            assert_eq!(start, 0);
        }
        other => panic!("unexpected command: {other:?}"),
    }
}

#[test]
fn parses_page_search_with_limit_and_start() {
    let cli = Cli::try_parse_from([
        "confluence", "page", "search", "--cql", "type=page", "--limit", "10", "--start", "25",
    ])
    .expect("should parse");

    match cli.command {
        Command::Page {
            command: PageCommand::Search { limit, start, .. },
        } => {
            assert_eq!(limit, 10);
            assert_eq!(start, 25);
        }
        other => panic!("unexpected command: {other:?}"),
    }
}

#[test]
fn rejects_page_search_missing_cql() {
    let result = Cli::try_parse_from(["confluence", "page", "search"]);

    assert!(result.is_err());
}

// --- space list ---

#[test]
fn parses_space_list_defaults() {
    let cli = Cli::try_parse_from(["confluence", "space", "list"]).expect("should parse");

    match cli.command {
        Command::Space {
            command: SpaceCommand::List { limit, cursor },
        } => {
            assert_eq!(limit, 25);
            assert!(cursor.is_none());
        }
        other => panic!("unexpected command: {other:?}"),
    }
}

#[test]
fn parses_space_list_with_limit_and_cursor() {
    let cli = Cli::try_parse_from([
        "confluence", "space", "list", "--limit", "5", "--cursor", "abc123",
    ])
    .expect("should parse");

    match cli.command {
        Command::Space {
            command: SpaceCommand::List { limit, cursor },
        } => {
            assert_eq!(limit, 5);
            assert_eq!(cursor.as_deref(), Some("abc123"));
        }
        other => panic!("unexpected command: {other:?}"),
    }
}

// --- template create ---

#[test]
fn parses_template_create_with_body() {
    let cli = Cli::try_parse_from([
        "confluence", "template", "create", "--name", "Runbook Template",
        "--body", "<p>Steps</p>",
    ])
    .expect("should parse");

    match cli.command {
        Command::Template {
            command:
                TemplateCommand::Create {
                    space_key,
                    name,
                    description,
                    body,
                    body_file,
                },
        } => {
            assert!(space_key.is_none());
            assert_eq!(name, "Runbook Template");
            assert!(description.is_none());
            assert_eq!(body.as_deref(), Some("<p>Steps</p>"));
            assert!(body_file.is_none());
        }
        other => panic!("unexpected command: {other:?}"),
    }
}

#[test]
fn parses_template_create_with_body_file_space_key_and_description() {
    let cli = Cli::try_parse_from([
        "confluence", "template", "create", "--name", "Runbook Template",
        "--space-key", "ENG", "--description", "Standard runbook layout",
        "--body-file", "./runbook.html",
    ])
    .expect("should parse");

    match cli.command {
        Command::Template {
            command: TemplateCommand::Create { space_key, description, body_file, body, .. },
        } => {
            assert_eq!(space_key.as_deref(), Some("ENG"));
            assert_eq!(description.as_deref(), Some("Standard runbook layout"));
            assert_eq!(body_file.as_deref(), Some("./runbook.html"));
            assert!(body.is_none());
        }
        other => panic!("unexpected command: {other:?}"),
    }
}

#[test]
fn rejects_template_create_with_body_and_body_file_together() {
    let result = Cli::try_parse_from([
        "confluence", "template", "create", "--name", "X",
        "--body", "<p>a</p>", "--body-file", "./t.html",
    ]);

    assert!(result.is_err(), "--body and --body-file should conflict");
}

#[test]
fn rejects_template_create_missing_name() {
    let result = Cli::try_parse_from([
        "confluence", "template", "create", "--body", "<p>a</p>",
    ]);

    assert!(result.is_err());
}

// --- template list ---

#[test]
fn parses_template_list_defaults() {
    let cli = Cli::try_parse_from(["confluence", "template", "list"]).expect("should parse");

    match cli.command {
        Command::Template {
            command: TemplateCommand::List { space_key, limit, start },
        } => {
            assert!(space_key.is_none());
            assert_eq!(limit, 25);
            assert_eq!(start, 0);
        }
        other => panic!("unexpected command: {other:?}"),
    }
}

#[test]
fn parses_template_list_with_space_key_limit_and_start() {
    let cli = Cli::try_parse_from([
        "confluence", "template", "list", "--space-key", "ENG", "--limit", "10", "--start", "5",
    ])
    .expect("should parse");

    match cli.command {
        Command::Template {
            command: TemplateCommand::List { space_key, limit, start },
        } => {
            assert_eq!(space_key.as_deref(), Some("ENG"));
            assert_eq!(limit, 10);
            assert_eq!(start, 5);
        }
        other => panic!("unexpected command: {other:?}"),
    }
}

// --- template update ---

#[test]
fn parses_template_update_with_name_only() {
    let cli =
        Cli::try_parse_from(["confluence", "template", "update", "4321", "--name", "New name"])
            .expect("should parse");

    match cli.command {
        Command::Template {
            command:
                TemplateCommand::Update {
                    id,
                    name,
                    description,
                    body,
                    body_file,
                },
        } => {
            assert_eq!(id, "4321");
            assert_eq!(name.as_deref(), Some("New name"));
            assert!(description.is_none());
            assert!(body.is_none());
            assert!(body_file.is_none());
        }
        other => panic!("unexpected command: {other:?}"),
    }
}

#[test]
fn parses_template_update_with_body_file_and_description() {
    let cli = Cli::try_parse_from([
        "confluence", "template", "update", "4321",
        "--description", "New description", "--body-file", "./new.html",
    ])
    .expect("should parse");

    match cli.command {
        Command::Template {
            command: TemplateCommand::Update { description, body_file, body, .. },
        } => {
            assert_eq!(description.as_deref(), Some("New description"));
            assert_eq!(body_file.as_deref(), Some("./new.html"));
            assert!(body.is_none());
        }
        other => panic!("unexpected command: {other:?}"),
    }
}

#[test]
fn rejects_template_update_with_body_and_body_file_together() {
    let result = Cli::try_parse_from([
        "confluence", "template", "update", "4321",
        "--body", "<p>a</p>", "--body-file", "./t.html",
    ]);

    assert!(result.is_err(), "--body and --body-file should conflict");
}

#[test]
fn parses_template_update_with_no_flags() {
    // Parsing allows this; run_update's runtime check rejects it (nothing to update).
    let cli =
        Cli::try_parse_from(["confluence", "template", "update", "4321"]).expect("should parse");

    assert!(matches!(cli.command, Command::Template { command: TemplateCommand::Update { .. } }));
}

#[test]
fn rejects_template_update_without_id() {
    let result = Cli::try_parse_from(["confluence", "template", "update"]);

    assert!(result.is_err());
}

// --- template delete ---

#[test]
fn parses_template_delete_with_confirm() {
    let cli = Cli::try_parse_from(["confluence", "template", "delete", "4321", "--confirm"])
        .expect("should parse");

    match cli.command {
        Command::Template {
            command: TemplateCommand::Delete { id, confirm },
        } => {
            assert_eq!(id, "4321");
            assert!(confirm);
        }
        other => panic!("unexpected command: {other:?}"),
    }
}

#[test]
fn parses_template_delete_without_confirm_defaults_false() {
    let cli = Cli::try_parse_from(["confluence", "template", "delete", "4321"])
        .expect("should parse");

    match cli.command {
        Command::Template {
            command: TemplateCommand::Delete { confirm, .. },
        } => assert!(!confirm),
        other => panic!("unexpected command: {other:?}"),
    }
}

#[test]
fn rejects_template_delete_without_id() {
    let result = Cli::try_parse_from(["confluence", "template", "delete"]);

    assert!(result.is_err());
}
