#![allow(clippy::unwrap_used, clippy::expect_used)]

use serde_json::json;

use super::{parse_comma_separated_required, print_json};
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

#[test]
fn parse_comma_separated_required_splits_multiple_values() {
    let values = parse_comma_separated_required("a,b", "user").expect("should parse");

    assert_eq!(values, vec!["a".to_string(), "b".to_string()]);
}

#[test]
fn parse_comma_separated_required_trims_whitespace() {
    let values = parse_comma_separated_required(" a , b ", "user").expect("should parse");

    assert_eq!(values, vec!["a".to_string(), "b".to_string()]);
}

#[test]
fn parse_comma_separated_required_single_value_has_length_one() {
    let values = parse_comma_separated_required("a", "user").expect("should parse");

    assert_eq!(values.len(), 1);
    assert_eq!(values, vec!["a".to_string()]);
}

#[test]
fn parse_comma_separated_required_errors_on_empty_string() {
    let err = parse_comma_separated_required("", "user").expect_err("should error");

    match err {
        CliError::EmptyValueList { flag, value } => {
            assert_eq!(flag, "user");
            assert_eq!(value, "");
        }
        other => panic!("expected CliError::EmptyValueList, got {other:?}"),
    }
}

#[test]
fn parse_comma_separated_required_errors_when_only_commas() {
    let err = parse_comma_separated_required(",,", "event-type").expect_err("should error");

    match err {
        CliError::EmptyValueList { flag, value } => {
            assert_eq!(flag, "event-type");
            assert_eq!(value, ",,");
        }
        other => panic!("expected CliError::EmptyValueList, got {other:?}"),
    }
}
