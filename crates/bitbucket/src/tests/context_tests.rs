#![allow(clippy::unwrap_used, clippy::expect_used)]

use serde_json::json;

use super::{print_json, split_repository};
use crate::error::CliError;
use cli_fields::{RenderError, Select};

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
fn required_select_returns_select_error() {
    let value = json!({"uuid": "x", "display_name": "y"});

    let err = print_json(&value, Select::Required).expect_err("should require --select");
    match err {
        CliError::Select(RenderError::SelectRequired { size, available_fields }) => {
            assert!(size > 0);
            assert_eq!(available_fields, "top-level fields: display_name, uuid");
        }
        other => panic!("expected CliError::Select(SelectRequired), got {other:?}"),
    }
}

#[test]
fn select_all_still_succeeds() {
    let value = json!({"uuid": "x"});

    assert!(print_json(&value, Select::All).is_ok());
}

#[test]
fn non_empty_fields_still_succeeds() {
    let value = json!({"uuid": "x"});

    assert!(print_json(&value, Select::Fields(&["uuid"])).is_ok());
}
