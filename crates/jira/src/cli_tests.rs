#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::error;
use super::{AuthCommand, Cli, Command, CommentCommand, IssueCommand};
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

#[test]
fn parses_issue_comment_add() {
    let cli =
        Cli::try_parse_from(["jira", "issue", "comment", "add", "KAN-1", "--body", "hello"])
            .expect("should parse");

    match cli.command {
        Command::Issue {
            command:
                IssueCommand::Comment {
                    command: CommentCommand::Add { key, body },
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
    // Whether to reject at runtime is a separate concern (see BACKLOG).
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
    // BACKLOG FIELDS-1: trailing comma produces an empty segment after split in run().
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
            command: IssueCommand::Create { project, issue_type, summary, .. },
        } => {
            assert_eq!(project, "KAN");
            assert_eq!(issue_type, "Task");
            assert_eq!(summary, "Fix the bug");
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
    ])
    .expect("should parse");

    match cli.command {
        Command::Issue {
            command: IssueCommand::Create { description, assignee, priority, .. },
        } => {
            assert_eq!(description.as_deref(), Some("Steps to reproduce"));
            assert_eq!(assignee.as_deref(), Some("account-id-123"));
            assert_eq!(priority.as_deref(), Some("High"));
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
    // Documents current behaviour; see BACKLOG CREATE-1.
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
            command: IssueCommand::Search { jql, max_results, page_token, fields },
        } => {
            assert_eq!(jql, "project=KAN AND status=\"In Progress\"");
            assert_eq!(max_results, 50);
            assert!(page_token.is_none());
            assert!(fields.is_none());
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
