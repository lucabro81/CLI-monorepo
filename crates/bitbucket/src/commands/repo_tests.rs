#![allow(clippy::unwrap_used, clippy::expect_used)]

use super::split_repository;
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
