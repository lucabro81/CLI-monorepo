#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::error;
use super::{AuthCommand, Cli, Command, CommentCommand, IssueCommand, ProjectCommand, UserCommand};
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

    assert!(!cli.user, "default should be service account (client_credentials)");
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
    let cli = Cli::try_parse_from(["jira", "auth", "login", "--user"]).expect("should parse");

    assert!(cli.user, "--user should select the interactive 3LO flow");
    match cli.command {
        Command::Auth {
            command: AuthCommand::Login { remote, .. },
        } => assert!(!remote),
        other => panic!("unexpected command: {other:?}"),
    }
}

fn login_error_kind(args: &[&str]) -> clap::error::ErrorKind {
    Cli::try_parse_from(["jira", "auth", "login"].iter().chain(args)).unwrap_err().kind()
}

#[test]
fn parses_auth_login_remote_start() {
    let cli = Cli::try_parse_from([
        "jira", "auth", "login", "--user", "--remote", "--redirect-uri", "https://m.example/cb",
    ])
    .expect("should parse");

    assert!(cli.user);
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
    let cli = Cli::try_parse_from(["jira", "auth", "login", "--user", "--code", "c1", "--state", "s1"])
        .expect("should parse");

    assert!(cli.user);
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
    assert_eq!(login_error_kind(&["--user", "--remote"]), MissingRequiredArgument);
    assert_eq!(login_error_kind(&["--user", "--redirect-uri", "https://m/cb"]), MissingRequiredArgument);
    assert_eq!(login_error_kind(&["--user", "--code", "c"]), MissingRequiredArgument);
    assert_eq!(login_error_kind(&["--user", "--state", "s"]), MissingRequiredArgument);
    assert_eq!(
        login_error_kind(&["--user", "--remote", "--redirect-uri", "https://m/cb", "--code", "c", "--state", "s"]),
        ArgumentConflict
    );
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

// --- global --user (issue #164) ---

#[test]
fn user_flag_defaults_to_the_service_identity() {
    let cli = Cli::try_parse_from(["jira", "issue", "get", "PROJ-1"]).expect("should parse");

    assert!(!cli.user);
}

#[test]
fn user_flag_is_accepted_after_any_subcommand() {
    for args in [
        &["jira", "issue", "get", "PROJ-1", "--user"][..],
        &["jira", "issue", "comment", "add", "PROJ-1", "--body", "hi", "--user"],
        &["jira", "user", "search", "--query", "ann", "--user"],
        &["jira", "project", "search", "--query", "mer", "--user"],
        &["jira", "auth", "whoami", "--user"],
        &["jira", "doctor", "--user"],
        &["jira", "init", "--user"],
    ] {
        let cli = Cli::try_parse_from(args).unwrap_or_else(|e| panic!("{args:?}: {e}"));
        assert!(cli.user, "{args:?} should select the human identity");
    }
}

#[test]
fn user_flag_is_accepted_before_the_subcommand() {
    let cli = Cli::try_parse_from(["jira", "--user", "issue", "get", "PROJ-1"]).expect("should parse");

    assert!(cli.user);
}

#[test]
fn remote_login_flags_accept_a_user_flag_placed_before_the_subcommand() {
    // --remote/--code require the global --user wherever it is written.
    let cli = Cli::try_parse_from(["jira", "--user", "auth", "login", "--code", "c", "--state", "s"])
        .expect("should parse");

    assert!(cli.user);
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

#[test]
fn parses_issue_comment_add() {
    let cli =
        Cli::try_parse_from(["jira", "issue", "comment", "add", "KAN-1", "--body", "hello"])
            .expect("should parse");

    match cli.command {
        Command::Issue {
            command:
                IssueCommand::Comment {
                    command: CommentCommand::Add { key, body, .. },
                },
        } => {
            assert_eq!(key, "KAN-1");
            assert_eq!(body, "hello");
        }
        other => panic!("unexpected command: {other:?}"),
    }
}

#[test]
fn parses_issue_comment_remove() {
    let cli =
        Cli::try_parse_from(["jira", "issue", "comment", "remove", "KAN-1", "comment-42"])
            .expect("should parse");

    match cli.command {
        Command::Issue {
            command:
                IssueCommand::Comment {
                    command: CommentCommand::Remove { key, id },
                },
        } => {
            assert_eq!(key, "KAN-1");
            assert_eq!(id, "comment-42");
        }
        other => panic!("unexpected command: {other:?}"),
    }
}

#[test]
fn parses_issue_comment_add_with_mention_flag() {
    let cli = Cli::try_parse_from([
        "jira", "issue", "comment", "add", "KAN-1", "--body", "can you check?", "--mention",
        "5b10ac8d82e05b22cc7d4ef5",
    ])
    .expect("should parse");

    match cli.command {
        Command::Issue {
            command:
                IssueCommand::Comment {
                    command: CommentCommand::Add { key, body, mention },
                },
        } => {
            assert_eq!(key, "KAN-1");
            assert_eq!(body, "can you check?");
            assert_eq!(mention.as_deref(), Some("5b10ac8d82e05b22cc7d4ef5"));
        }
        other => panic!("unexpected command: {other:?}"),
    }
}

#[test]
fn parses_issue_comment_add_without_mention_flag_defaults_to_none() {
    let cli =
        Cli::try_parse_from(["jira", "issue", "comment", "add", "KAN-1", "--body", "hello"])
            .expect("should parse");

    match cli.command {
        Command::Issue {
            command:
                IssueCommand::Comment {
                    command: CommentCommand::Add { mention, .. },
                },
        } => assert_eq!(mention, None),
        other => panic!("unexpected command: {other:?}"),
    }
}

#[test]
fn parses_issue_comment_add_with_inline_mention_placeholder_in_body() {
    // The {{mention:ACCOUNT_ID}} placeholder is just part of the --body string at the
    // clap-parsing level; it is parsed into an ADF mention node later by the handler.
    let cli = Cli::try_parse_from([
        "jira", "issue", "comment", "add", "KAN-1", "--body",
        "Thanks {{mention:5b10ac8d82e05b22cc7d4ef5}} for the fix",
    ])
    .expect("should parse");

    match cli.command {
        Command::Issue {
            command:
                IssueCommand::Comment {
                    command: CommentCommand::Add { body, .. },
                },
        } => assert_eq!(body, "Thanks {{mention:5b10ac8d82e05b22cc7d4ef5}} for the fix"),
        other => panic!("unexpected command: {other:?}"),
    }
}

#[test]
fn rejects_comment_add_missing_body_flag() {
    // --body is required; omitting it must fail
    let result = Cli::try_parse_from(["jira", "issue", "comment", "add", "KAN-1"]);

    assert!(result.is_err());
}

#[test]
fn rejects_comment_add_missing_key() {
    let result = Cli::try_parse_from(["jira", "issue", "comment", "add", "--body", "hello"]);

    assert!(result.is_err());
}

#[test]
fn rejects_comment_remove_missing_id() {
    let result = Cli::try_parse_from(["jira", "issue", "comment", "remove", "KAN-1"]);

    assert!(result.is_err());
}

#[test]
fn parses_issue_transition() {
    let cli =
        Cli::try_parse_from(["jira", "issue", "transition", "KAN-4", "--to", "In Progress"])
            .expect("should parse");

    match cli.command {
        Command::Issue {
            command: IssueCommand::Transition { key, to },
        } => {
            assert_eq!(key, "KAN-4");
            assert_eq!(to, "In Progress");
        }
        other => panic!("unexpected command: {other:?}"),
    }
}

#[test]
fn rejects_transition_missing_to_flag() {
    let result = Cli::try_parse_from(["jira", "issue", "transition", "KAN-4"]);

    assert!(result.is_err());
}

#[test]
fn rejects_transition_missing_key() {
    let result = Cli::try_parse_from(["jira", "issue", "transition", "--to", "Done"]);

    assert!(result.is_err());
}

#[test]
fn parses_issue_transitions_list() {
    let cli = Cli::try_parse_from(["jira", "issue", "transitions", "KAN-4"])
        .expect("should parse");

    match cli.command {
        Command::Issue {
            command: IssueCommand::Transitions { key },
        } => assert_eq!(key, "KAN-4"),
        other => panic!("unexpected command: {other:?}"),
    }
}

#[test]
fn rejects_transitions_list_missing_key() {
    let result = Cli::try_parse_from(["jira", "issue", "transitions"]);

    assert!(result.is_err());
}

#[test]
fn parses_fields_flag_on_issue_get() {
    let cli =
        Cli::try_parse_from(["jira", "--select", "summary,status.name", "issue", "get", "KAN-4"])
            .expect("should parse");

    assert_eq!(cli.select.as_deref(), Some("summary,status.name"));
}

#[test]
fn fields_flag_is_none_when_absent() {
    let cli = Cli::try_parse_from(["jira", "issue", "get", "KAN-4"]).expect("should parse");

    assert!(cli.select.is_none());
    assert!(!cli.select_all);
}

#[test]
fn select_all_flag_parses() {
    let cli = Cli::try_parse_from(["jira", "--select-all", "issue", "get", "KAN-4"])
        .expect("should parse");

    assert!(cli.select_all);
    assert!(cli.select.is_none());
}

#[test]
fn select_and_select_all_together_are_rejected() {
    let result = Cli::try_parse_from([
        "jira",
        "--select",
        "summary",
        "--select-all",
        "issue",
        "get",
        "KAN-4",
    ]);

    assert!(result.is_err(), "--select and --select-all should conflict");
}

#[test]
fn fields_flag_accepted_after_subcommand() {
    // global flag can appear after the subcommand too
    let cli = Cli::try_parse_from([
        "jira",
        "issue",
        "transitions",
        "KAN-4",
        "--select",
        "transitions.name",
    ])
    .expect("should parse");

    assert_eq!(cli.select.as_deref(), Some("transitions.name"));
}

#[test]
fn comment_add_accepts_empty_body() {
    // clap does not reject empty strings — an LLM could pass --body "".
    // Whether to reject at runtime is a separate concern, not currently tracked.
    let cli = Cli::try_parse_from(["jira", "issue", "comment", "add", "KAN-1", "--body", ""])
        .expect("should parse");

    match cli.command {
        Command::Issue {
            command: IssueCommand::Comment {
                command: CommentCommand::Add { body, .. },
            },
        } => assert_eq!(body, ""),
        other => panic!("unexpected: {other:?}"),
    }
}

#[test]
fn fields_flag_with_trailing_comma_parses_as_string() {
    // issue #66: trailing comma produces an empty segment after split in run().
    // This test documents that clap accepts the raw string; trimming/filtering is run()'s job.
    let cli =
        Cli::try_parse_from(["jira", "issue", "get", "KAN-4", "--select", "summary,"])
            .expect("should parse");

    assert_eq!(cli.select.as_deref(), Some("summary,"));
}

#[test]
fn fields_flag_with_spaces_around_comma_parses_as_string() {
    // Spaces are preserved by clap; run() uses str::trim on each segment.
    let cli = Cli::try_parse_from([
        "jira",
        "issue",
        "get",
        "KAN-4",
        "--select",
        "summary, status.name",
    ])
    .expect("should parse");

    assert_eq!(cli.select.as_deref(), Some("summary, status.name"));
}

// --- issue create ---

#[test]
fn parses_issue_create_with_required_fields() {
    let cli = Cli::try_parse_from([
        "jira", "issue", "create",
        "--project", "KAN",
        "--type", "Task",
        "--summary", "Fix the bug",
    ])
    .expect("should parse");

    match cli.command {
        Command::Issue {
            command: IssueCommand::Create { project, issue_type, summary, parent, .. },
        } => {
            assert_eq!(project, "KAN");
            assert_eq!(issue_type, "Task");
            assert_eq!(summary, "Fix the bug");
            assert_eq!(parent, None);
        }
        other => panic!("unexpected: {other:?}"),
    }
}

#[test]
fn parses_issue_create_with_all_optional_fields() {
    let cli = Cli::try_parse_from([
        "jira", "issue", "create",
        "--project", "KAN",
        "--type", "Bug",
        "--summary", "Login broken",
        "--description", "Steps to reproduce",
        "--assignee", "account-id-123",
        "--priority", "High",
        "--parent", "KAN-10",
    ])
    .expect("should parse");

    match cli.command {
        Command::Issue {
            command: IssueCommand::Create { description, assignee, priority, parent, .. },
        } => {
            assert_eq!(description.as_deref(), Some("Steps to reproduce"));
            assert_eq!(assignee.as_deref(), Some("account-id-123"));
            assert_eq!(priority.as_deref(), Some("High"));
            assert_eq!(parent.as_deref(), Some("KAN-10"));
        }
        other => panic!("unexpected: {other:?}"),
    }
}

#[test]
fn rejects_issue_create_missing_project() {
    let result = Cli::try_parse_from([
        "jira", "issue", "create",
        "--type", "Task", "--summary", "x",
    ]);
    assert!(result.is_err());
}

#[test]
fn rejects_issue_create_missing_type() {
    let result = Cli::try_parse_from([
        "jira", "issue", "create",
        "--project", "KAN", "--summary", "x",
    ]);
    assert!(result.is_err());
}

#[test]
fn rejects_issue_create_missing_summary() {
    let result = Cli::try_parse_from([
        "jira", "issue", "create",
        "--project", "KAN", "--type", "Task",
    ]);
    assert!(result.is_err());
}

// --- issue delete ---

#[test]
fn parses_issue_delete_with_confirm() {
    let cli =
        Cli::try_parse_from(["jira", "issue", "delete", "KAN-5", "--confirm"])
            .expect("should parse");

    match cli.command {
        Command::Issue {
            command: IssueCommand::Delete { key, confirm, delete_subtasks },
        } => {
            assert_eq!(key, "KAN-5");
            assert!(confirm);
            assert!(!delete_subtasks);
        }
        other => panic!("unexpected: {other:?}"),
    }
}

#[test]
fn parses_issue_delete_without_confirm_defaults_false() {
    // --confirm absent → confirm=false; runtime (not clap) rejects execution.
    let cli = Cli::try_parse_from(["jira", "issue", "delete", "KAN-5"]).expect("should parse");

    match cli.command {
        Command::Issue {
            command: IssueCommand::Delete { confirm, .. },
        } => assert!(!confirm),
        other => panic!("unexpected: {other:?}"),
    }
}

#[test]
fn parses_issue_delete_with_delete_subtasks() {
    let cli = Cli::try_parse_from([
        "jira", "issue", "delete", "KAN-5", "--confirm", "--delete-subtasks",
    ])
    .expect("should parse");

    match cli.command {
        Command::Issue {
            command: IssueCommand::Delete { delete_subtasks, .. },
        } => assert!(delete_subtasks),
        other => panic!("unexpected: {other:?}"),
    }
}

#[test]
fn rejects_issue_delete_missing_key() {
    let result = Cli::try_parse_from(["jira", "issue", "delete", "--confirm"]);
    assert!(result.is_err());
}

#[test]
fn issue_create_accepts_empty_summary() {
    // clap does not reject empty strings — Jira will return 400 at runtime.
    // Documents current behaviour; see issue #71.
    let cli = Cli::try_parse_from([
        "jira", "issue", "create",
        "--project", "KAN", "--type", "Task", "--summary", "",
    ])
    .expect("should parse");

    match cli.command {
        Command::Issue {
            command: IssueCommand::Create { summary, .. },
        } => assert_eq!(summary, ""),
        other => panic!("unexpected: {other:?}"),
    }
}

// --- issue assign ---

#[test]
fn parses_issue_assign_with_assignee() {
    let cli = Cli::try_parse_from([
        "jira", "issue", "assign", "KAN-5", "--assignee", "account-id-123",
    ])
    .expect("should parse");

    match cli.command {
        Command::Issue {
            command: IssueCommand::Assign { key, assignee, unassign },
        } => {
            assert_eq!(key, "KAN-5");
            assert_eq!(assignee.as_deref(), Some("account-id-123"));
            assert!(!unassign);
        }
        other => panic!("unexpected: {other:?}"),
    }
}

#[test]
fn parses_issue_assign_with_unassign_flag() {
    let cli = Cli::try_parse_from(["jira", "issue", "assign", "KAN-5", "--unassign"])
        .expect("should parse");

    match cli.command {
        Command::Issue {
            command: IssueCommand::Assign { key, assignee, unassign },
        } => {
            assert_eq!(key, "KAN-5");
            assert_eq!(assignee, None);
            assert!(unassign);
        }
        other => panic!("unexpected: {other:?}"),
    }
}

#[test]
fn rejects_issue_assign_with_both_assignee_and_unassign() {
    // --assignee and --unassign are mutually exclusive (clap `conflicts_with`) —
    // passing both is ambiguous about which action the caller actually wants.
    let result = Cli::try_parse_from([
        "jira", "issue", "assign", "KAN-5", "--assignee", "account-id-123", "--unassign",
    ]);
    assert!(result.is_err());
}

#[test]
fn rejects_issue_assign_missing_key() {
    let result = Cli::try_parse_from(["jira", "issue", "assign", "--assignee", "account-id-123"]);
    assert!(result.is_err());
}

// --- init ---

#[test]
fn parses_init_no_flags() {
    let cli = Cli::try_parse_from(["jira", "init"]).expect("should parse");

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
        "jira", "init", "--client-id", "abc123", "--client-secret", "s3cr3t",
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
    let cli = Cli::try_parse_from(["jira", "init", "--client-id", "abc123"])
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
    let cli = Cli::try_parse_from(["jira", "init", "--client-secret", "s3cr3t"])
        .expect("should parse");

    match cli.command {
        Command::Init { client_id, client_secret } => {
            assert!(client_id.is_none());
            assert_eq!(client_secret.as_deref(), Some("s3cr3t"));
        }
        other => panic!("unexpected: {other:?}"),
    }
}

// --- doctor ---

#[test]
fn parses_doctor() {
    let cli = Cli::try_parse_from(["jira", "doctor"]).expect("should parse");

    assert!(matches!(cli.command, Command::Doctor));
}

#[test]
fn doctor_with_select_flag() {
    let cli = Cli::try_parse_from([
        "jira", "doctor", "--select", "app_config.status,credentials.status",
    ])
    .expect("should parse");

    assert!(matches!(cli.command, Command::Doctor));
    assert_eq!(cli.select.as_deref(), Some("app_config.status,credentials.status"));
}

// --- issue search ---

#[test]
fn parses_issue_search_with_jql() {
    let cli = Cli::try_parse_from([
        "jira", "issue", "search", "--jql", "project=KAN AND status=\"In Progress\"",
    ])
    .expect("should parse");

    match cli.command {
        Command::Issue {
            command: IssueCommand::Search { jql, max_results, page_token, fields, stale_days },
        } => {
            assert_eq!(jql, "project=KAN AND status=\"In Progress\"");
            assert_eq!(max_results, 50);
            assert!(page_token.is_none());
            assert!(fields.is_none());
            assert!(stale_days.is_none());
        }
        other => panic!("unexpected: {other:?}"),
    }
}

#[test]
fn parses_issue_search_with_stale_days() {
    let cli = Cli::try_parse_from([
        "jira", "issue", "search", "--jql", "project=KAN", "--stale-days", "14",
    ])
    .expect("should parse");

    match cli.command {
        Command::Issue { command: IssueCommand::Search { stale_days, .. } } => {
            assert_eq!(stale_days, Some(14));
        }
        other => panic!("unexpected: {other:?}"),
    }
}

#[test]
fn parses_issue_search_with_max_results() {
    let cli = Cli::try_parse_from([
        "jira", "issue", "search", "--jql", "project=KAN", "--max-results", "10",
    ])
    .expect("should parse");

    match cli.command {
        Command::Issue { command: IssueCommand::Search { max_results, .. } } => {
            assert_eq!(max_results, 10);
        }
        other => panic!("unexpected: {other:?}"),
    }
}

#[test]
fn parses_issue_search_with_page_token() {
    let cli = Cli::try_parse_from([
        "jira", "issue", "search", "--jql", "project=KAN", "--page-token", "abc123",
    ])
    .expect("should parse");

    match cli.command {
        Command::Issue { command: IssueCommand::Search { page_token, .. } } => {
            assert_eq!(page_token.as_deref(), Some("abc123"));
        }
        other => panic!("unexpected: {other:?}"),
    }
}

#[test]
fn parses_issue_search_with_fields() {
    let cli = Cli::try_parse_from([
        "jira", "issue", "search", "--jql", "project=KAN", "--fields", "summary,status,priority",
    ])
    .expect("should parse");

    match cli.command {
        Command::Issue { command: IssueCommand::Search { fields, .. } } => {
            assert_eq!(fields.as_deref(), Some("summary,status,priority"));
        }
        other => panic!("unexpected: {other:?}"),
    }
}

#[test]
fn rejects_issue_search_missing_jql() {
    let result = Cli::try_parse_from(["jira", "issue", "search"]);
    assert!(result.is_err());
}

#[test]
fn parses_user_search_with_query() {
    let cli = Cli::try_parse_from(["jira", "user", "search", "--query", "Jane Doe"])
        .expect("should parse");

    match cli.command {
        Command::User { command: UserCommand::Search { query } } => {
            assert_eq!(query, "Jane Doe");
        }
        other => panic!("unexpected command: {other:?}"),
    }
}

#[test]
fn rejects_user_search_missing_query() {
    let result = Cli::try_parse_from(["jira", "user", "search"]);
    assert!(result.is_err());
}

#[test]
fn parses_project_search_with_query() {
    let cli = Cli::try_parse_from(["jira", "project", "search", "--query", "Mercury"])
        .expect("should parse");

    match cli.command {
        Command::Project { command: ProjectCommand::Search { query } } => {
            assert_eq!(query, "Mercury");
        }
        other => panic!("unexpected command: {other:?}"),
    }
}

#[test]
fn rejects_project_search_missing_query() {
    let result = Cli::try_parse_from(["jira", "project", "search"]);
    assert!(result.is_err());
}

#[test]
fn delete_not_confirmed_error_message_contains_key_and_corrective_command() {
    // Regression guard: if the error message format changes, an LLM can no longer
    // self-correct by reading the error and retrying with --confirm.
    use error::CliError;
    let err = CliError::DeleteNotConfirmed { key: "KAN-99".to_string() };
    let msg = err.to_string();

    assert!(msg.contains("KAN-99"), "error must name the key");
    assert!(msg.contains("--confirm"), "error must mention the --confirm flag");
    assert!(
        msg.contains("jira issue delete KAN-99 --confirm"),
        "error must include the exact command to run"
    );
}

#[test]
fn assign_missing_target_error_message_contains_key_and_both_retry_flags() {
    // Regression guard: an LLM must be able to self-correct from this message alone —
    // it should learn both valid retries (--assignee or --unassign) for the same key.
    use error::CliError;
    let err = CliError::AssignMissingTarget { key: "KAN-99".to_string() };
    let msg = err.to_string();

    assert!(msg.contains("KAN-99"), "error must name the key");
    assert!(msg.contains("--assignee"), "error must mention the --assignee flag");
    assert!(msg.contains("--unassign"), "error must mention the --unassign flag");
    assert!(
        msg.contains("jira issue assign KAN-99 --assignee") && msg.contains("jira issue assign KAN-99 --unassign"),
        "error must include both exact retry commands"
    );
}

/// Commands whose output is NOT exempt from the mandatory `--select` (they call
/// `print_json(&value, select)`, not `select.or_all()`). Keep in sync with the
/// "Exempt?" table in this crate's CLAUDE.md.
const NON_EXEMPT_COMMANDS: &[&[&str]] = &[
    &["issue", "get"],
    &["issue", "search"],
    &["user", "search"],
    &["project", "search"],
];

/// Lines of a command's `after_help` that are runnable example invocations.
fn help_example_lines(path: &[&str]) -> Vec<String> {
    use clap::CommandFactory;

    let mut command = Cli::command();
    for name in path {
        command = command
            .find_subcommand(name)
            .unwrap_or_else(|| panic!("no subcommand {name:?} in {path:?}"))
            .clone();
    }
    let after_help = command
        .get_after_help()
        .unwrap_or_else(|| panic!("{path:?} has no after_help examples"))
        .to_string();

    after_help
        .lines()
        .map(|line| line.trim().trim_start_matches("Example:").trim())
        .filter(|line| line.starts_with("jira "))
        .map(str::to_string)
        .collect()
}

#[test]
fn no_help_example_depends_on_the_authenticated_identity() {
    // Regression test for issue #138: `issue search --help` showed
    // `assignee=currentUser()`. currentUser() resolves to whoever the CLI is
    // authenticated as — the bot/service account when an agent runs it on
    // someone's behalf — so a model copying the example silently searched the
    // bot's issues instead of the person's.
    // Only example invocations are checked: prose in the same help text may
    // name currentUser() to warn against it.
    fn collect_examples(command: &clap::Command, out: &mut Vec<String>) {
        if let Some(after_help) = command.get_after_help() {
            out.extend(
                after_help
                    .to_string()
                    .lines()
                    .map(str::trim)
                    .filter(|line| line.starts_with("jira "))
                    .map(str::to_string),
            );
        }
        for sub in command.get_subcommands() {
            collect_examples(sub, out);
        }
    }
    use clap::CommandFactory;

    let mut examples = Vec::new();
    collect_examples(&Cli::command(), &mut examples);
    assert!(
        examples.iter().any(|e| e.starts_with("jira issue search ")),
        "example collection must reach nested subcommands like issue search"
    );

    for example in examples {
        assert!(
            !example.to_lowercase().contains("currentuser()"),
            "help example depends on the authenticated identity: {example}"
        );
    }
}

#[test]
fn every_help_example_of_a_non_exempt_command_passes_select() {
    // Regression test for issue #133: after --select became mandatory, the
    // --help examples of non-exempt commands kept showing invocations without
    // it, which the CLI refuses to run — an agent copying them got an error.
    for path in NON_EXEMPT_COMMANDS {
        let examples = help_example_lines(path);
        assert!(!examples.is_empty(), "{path:?} has no example lines");

        for example in examples {
            assert!(
                example.contains("--select ") || example.contains("--select-all"),
                "{path:?} help example runs without --select and would be refused: {example}"
            );
        }
    }
}
