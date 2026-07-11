#![allow(clippy::unwrap_used, clippy::expect_used)]

use serde_json::json;

use super::print_json;
use crate::error::CliError;
use cli_fields::{RenderError, Select};

#[test]
fn required_select_returns_select_error() {
    let value = json!({"name": "spaces/1", "displayName": "General"});

    let err = print_json(&value, Select::Required).expect_err("should require --select");
    match err {
        CliError::Select(RenderError::SelectRequired { size, available_fields }) => {
            assert!(size > 0);
            assert_eq!(available_fields, "top-level fields: displayName, name");
        }
        other => panic!("expected CliError::Select(SelectRequired), got {other:?}"),
    }
}

#[test]
fn select_all_still_succeeds() {
    let value = json!({"name": "spaces/1"});

    assert!(print_json(&value, Select::All).is_ok());
}

#[test]
fn non_empty_fields_still_succeeds() {
    let value = json!({"name": "spaces/1"});

    assert!(print_json(&value, Select::Fields(&["name"])).is_ok());
}
