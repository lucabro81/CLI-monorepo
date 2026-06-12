#![allow(clippy::unwrap_used, clippy::expect_used)]

use super::{AuthCommand, BranchCommand, Cli, Command, PrCommand, RepoCommand};
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

#[test]
fn parses_repo_create_with_no_optional_flags() {
    let cli = Cli::try_parse_from(["bitbucket", "repo", "create", "lucabrognaracode/my-repo"]).expect("should parse");

    match cli.command {
        Command::Repo {
            command: RepoCommand::Create { repository, description, private, project },
        } => {
            assert_eq!(repository, "lucabrognaracode/my-repo");
            assert_eq!(description, None);
            assert!(!private);
            assert_eq!(project, None);
        }
        other => panic!("expected Repo Create, got {other:?}"),
    }
}

#[test]
fn parses_repo_create_with_all_flags() {
    let cli = Cli::try_parse_from([
        "bitbucket", "repo", "create", "lucabrognaracode/my-repo",
        "--description", "my repo",
        "--private",
        "--project", "PROJ",
    ]).expect("should parse");

    match cli.command {
        Command::Repo {
            command: RepoCommand::Create { repository, description, private, project },
        } => {
            assert_eq!(repository, "lucabrognaracode/my-repo");
            assert_eq!(description, Some("my repo".to_string()));
            assert!(private);
            assert_eq!(project, Some("PROJ".to_string()));
        }
        other => panic!("expected Repo Create, got {other:?}"),
    }
}

#[test]
fn parses_pr_list_with_no_optional_flags() {
    let cli = Cli::try_parse_from(["bitbucket", "pr", "list", "lucabrognaracode/my-repo"]).expect("should parse");

    match cli.command {
        Command::Pr {
            command: PrCommand::List { repository, state, page },
        } => {
            assert_eq!(repository, "lucabrognaracode/my-repo");
            assert_eq!(state, None);
            assert_eq!(page, None);
        }
        other => panic!("expected Pr List, got {other:?}"),
    }
}

#[test]
fn parses_pr_list_with_all_flags() {
    let cli = Cli::try_parse_from([
        "bitbucket", "pr", "list", "lucabrognaracode/my-repo",
        "--state", "MERGED",
        "--page", "2",
    ]).expect("should parse");

    match cli.command {
        Command::Pr {
            command: PrCommand::List { repository, state, page },
        } => {
            assert_eq!(repository, "lucabrognaracode/my-repo");
            assert_eq!(state, Some("MERGED".to_string()));
            assert_eq!(page, Some(2));
        }
        other => panic!("expected Pr List, got {other:?}"),
    }
}

#[test]
fn parses_pr_create_with_no_optional_flags() {
    let cli = Cli::try_parse_from([
        "bitbucket", "pr", "create", "lucabrognaracode/my-repo",
        "--title", "My PR",
        "--source", "feature-branch",
    ]).expect("should parse");

    match cli.command {
        Command::Pr {
            command: PrCommand::Create { repository, title, source, destination, description, close_source_branch },
        } => {
            assert_eq!(repository, "lucabrognaracode/my-repo");
            assert_eq!(title, "My PR");
            assert_eq!(source, "feature-branch");
            assert_eq!(destination, None);
            assert_eq!(description, None);
            assert!(!close_source_branch);
        }
        other => panic!("expected Pr Create, got {other:?}"),
    }
}

#[test]
fn parses_pr_create_with_all_flags() {
    let cli = Cli::try_parse_from([
        "bitbucket", "pr", "create", "lucabrognaracode/my-repo",
        "--title", "My PR",
        "--source", "feature-branch",
        "--destination", "main",
        "--description", "does things",
        "--close-source-branch",
    ]).expect("should parse");

    match cli.command {
        Command::Pr {
            command: PrCommand::Create { repository, title, source, destination, description, close_source_branch },
        } => {
            assert_eq!(repository, "lucabrognaracode/my-repo");
            assert_eq!(title, "My PR");
            assert_eq!(source, "feature-branch");
            assert_eq!(destination, Some("main".to_string()));
            assert_eq!(description, Some("does things".to_string()));
            assert!(close_source_branch);
        }
        other => panic!("expected Pr Create, got {other:?}"),
    }
}

#[test]
fn parses_pr_comment_with_no_optional_flags() {
    let cli = Cli::try_parse_from([
        "bitbucket", "pr", "comment", "lucabrognaracode/my-repo", "42",
        "--content", "Looks good to me",
    ]).expect("should parse");

    match cli.command {
        Command::Pr {
            command: PrCommand::Comment { repository, id, content, path, line },
        } => {
            assert_eq!(repository, "lucabrognaracode/my-repo");
            assert_eq!(id, 42);
            assert_eq!(content, "Looks good to me");
            assert_eq!(path, None);
            assert_eq!(line, None);
        }
        other => panic!("expected Pr Comment, got {other:?}"),
    }
}

#[test]
fn parses_pr_comment_with_inline_flags() {
    let cli = Cli::try_parse_from([
        "bitbucket", "pr", "comment", "lucabrognaracode/my-repo", "42",
        "--content", "Fix this",
        "--path", "src/main.rs",
        "--line", "10",
    ]).expect("should parse");

    match cli.command {
        Command::Pr {
            command: PrCommand::Comment { repository, id, content, path, line },
        } => {
            assert_eq!(repository, "lucabrognaracode/my-repo");
            assert_eq!(id, 42);
            assert_eq!(content, "Fix this");
            assert_eq!(path, Some("src/main.rs".to_string()));
            assert_eq!(line, Some(10));
        }
        other => panic!("expected Pr Comment, got {other:?}"),
    }
}

#[test]
fn parses_branch_list_without_page() {
    let cli = Cli::try_parse_from(["bitbucket", "branch", "list", "lucabrognaracode/my-repo"]).expect("should parse");

    match cli.command {
        Command::Branch {
            command: BranchCommand::List { repository, page },
        } => {
            assert_eq!(repository, "lucabrognaracode/my-repo");
            assert_eq!(page, None);
        }
        other => panic!("expected Branch List, got {other:?}"),
    }
}

#[test]
fn parses_branch_list_with_page() {
    let cli = Cli::try_parse_from(["bitbucket", "branch", "list", "lucabrognaracode/my-repo", "--page", "2"]).expect("should parse");

    match cli.command {
        Command::Branch {
            command: BranchCommand::List { repository, page },
        } => {
            assert_eq!(repository, "lucabrognaracode/my-repo");
            assert_eq!(page, Some(2));
        }
        other => panic!("expected Branch List, got {other:?}"),
    }
}

#[test]
fn parses_pr_approve() {
    let cli = Cli::try_parse_from(["bitbucket", "pr", "approve", "lucabrognaracode/my-repo", "42"]).expect("should parse");

    match cli.command {
        Command::Pr {
            command: PrCommand::Approve { repository, id },
        } => {
            assert_eq!(repository, "lucabrognaracode/my-repo");
            assert_eq!(id, 42);
        }
        other => panic!("expected Pr Approve, got {other:?}"),
    }
}

#[test]
fn parses_pr_unapprove() {
    let cli = Cli::try_parse_from(["bitbucket", "pr", "unapprove", "lucabrognaracode/my-repo", "42"]).expect("should parse");

    match cli.command {
        Command::Pr {
            command: PrCommand::Unapprove { repository, id },
        } => {
            assert_eq!(repository, "lucabrognaracode/my-repo");
            assert_eq!(id, 42);
        }
        other => panic!("expected Pr Unapprove, got {other:?}"),
    }
}

#[test]
fn parses_pr_decline_without_confirm() {
    let cli = Cli::try_parse_from(["bitbucket", "pr", "decline", "lucabrognaracode/my-repo", "42"]).expect("should parse");

    match cli.command {
        Command::Pr {
            command: PrCommand::Decline { repository, id, confirm },
        } => {
            assert_eq!(repository, "lucabrognaracode/my-repo");
            assert_eq!(id, 42);
            assert!(!confirm);
        }
        other => panic!("expected Pr Decline, got {other:?}"),
    }
}

#[test]
fn parses_pr_decline_with_confirm() {
    let cli = Cli::try_parse_from(["bitbucket", "pr", "decline", "lucabrognaracode/my-repo", "42", "--confirm"]).expect("should parse");

    match cli.command {
        Command::Pr {
            command: PrCommand::Decline { repository, id, confirm },
        } => {
            assert_eq!(repository, "lucabrognaracode/my-repo");
            assert_eq!(id, 42);
            assert!(confirm);
        }
        other => panic!("expected Pr Decline, got {other:?}"),
    }
}

#[test]
fn parses_pr_merge_with_no_optional_flags() {
    let cli = Cli::try_parse_from(["bitbucket", "pr", "merge", "lucabrognaracode/my-repo", "42"]).expect("should parse");

    match cli.command {
        Command::Pr {
            command: PrCommand::Merge { repository, id, message, merge_strategy, close_source_branch, confirm },
        } => {
            assert_eq!(repository, "lucabrognaracode/my-repo");
            assert_eq!(id, 42);
            assert_eq!(message, None);
            assert_eq!(merge_strategy, None);
            assert!(!close_source_branch);
            assert!(!confirm);
        }
        other => panic!("expected Pr Merge, got {other:?}"),
    }
}

#[test]
fn parses_pr_merge_with_all_flags() {
    let cli = Cli::try_parse_from([
        "bitbucket", "pr", "merge", "lucabrognaracode/my-repo", "42",
        "--message", "Merging feature",
        "--merge-strategy", "squash",
        "--close-source-branch",
        "--confirm",
    ]).expect("should parse");

    match cli.command {
        Command::Pr {
            command: PrCommand::Merge { repository, id, message, merge_strategy, close_source_branch, confirm },
        } => {
            assert_eq!(repository, "lucabrognaracode/my-repo");
            assert_eq!(id, 42);
            assert_eq!(message, Some("Merging feature".to_string()));
            assert_eq!(merge_strategy, Some("squash".to_string()));
            assert!(close_source_branch);
            assert!(confirm);
        }
        other => panic!("expected Pr Merge, got {other:?}"),
    }
}

#[test]
fn parses_pr_get() {
    let cli = Cli::try_parse_from(["bitbucket", "pr", "get", "lucabrognaracode/my-repo", "42"]).expect("should parse");

    match cli.command {
        Command::Pr {
            command: PrCommand::Get { repository, id },
        } => {
            assert_eq!(repository, "lucabrognaracode/my-repo");
            assert_eq!(id, 42);
        }
        other => panic!("expected Pr Get, got {other:?}"),
    }
}
