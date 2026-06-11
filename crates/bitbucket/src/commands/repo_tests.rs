#![allow(clippy::unwrap_used, clippy::expect_used)]

use super::{build_create_body, split_repository};
use crate::error::CliError;

#[test]
fn splits_workspace_and_repo_slug() {
    let (workspace, repo_slug) = split_repository("lucabrognaracode/my-repo").expect("should split");

    assert_eq!(workspace, "lucabrognaracode");
    assert_eq!(repo_slug, "my-repo");
}

#[test]
fn rejects_repository_without_slash() {
    let err = split_repository("my-repo").expect_err("should reject");

    assert!(matches!(err, CliError::InvalidRepository { value } if value == "my-repo"));
}

#[test]
fn rejects_repository_with_empty_workspace_or_slug() {
    assert!(matches!(
        split_repository("/my-repo"),
        Err(CliError::InvalidRepository { .. })
    ));
    assert!(matches!(
        split_repository("lucabrognaracode/"),
        Err(CliError::InvalidRepository { .. })
    ));
}

#[test]
fn build_create_body_defaults_to_git_scm_only() {
    let body = build_create_body(None, false, None);

    assert_eq!(body, serde_json::json!({"scm": "git"}));
}

#[test]
fn build_create_body_includes_description_when_set() {
    let body = build_create_body(Some("my repo".to_string()), false, None);

    assert_eq!(body, serde_json::json!({"scm": "git", "description": "my repo"}));
}

#[test]
fn build_create_body_includes_is_private_when_true() {
    let body = build_create_body(None, true, None);

    assert_eq!(body, serde_json::json!({"scm": "git", "is_private": true}));
}

#[test]
fn build_create_body_includes_project_key_when_set() {
    let body = build_create_body(None, false, Some("PROJ".to_string()));

    assert_eq!(body, serde_json::json!({"scm": "git", "project": {"key": "PROJ"}}));
}

#[test]
fn build_create_body_combines_all_fields() {
    let body = build_create_body(Some("desc".to_string()), true, Some("PROJ".to_string()));

    assert_eq!(
        body,
        serde_json::json!({
            "scm": "git",
            "description": "desc",
            "is_private": true,
            "project": {"key": "PROJ"}
        })
    );
}
