#![allow(clippy::unwrap_used, clippy::expect_used)]

use serde_json::json;

use super::{render_json, RenderError, Select};

#[test]
fn required_without_select_returns_select_required_error() {
    let value = json!({"summary": "x", "status": "open"});

    let err = render_json(&value, Select::Required).expect_err("should require --select");
    match err {
        RenderError::SelectRequired { size, available_fields } => {
            assert!(size > 0);
            assert_eq!(available_fields, "top-level fields: status, summary");
        }
        other @ RenderError::Serialize(_) => panic!("expected SelectRequired, got {other:?}"),
    }
}

#[test]
fn select_required_error_reports_actual_pretty_printed_byte_size() {
    let value = json!({"a": "b"});
    let expected_size = serde_json::to_string_pretty(&value).expect("serializes").len();

    let err = render_json(&value, Select::Required).expect_err("should require --select");
    match err {
        RenderError::SelectRequired { size, .. } => assert_eq!(size, expected_size),
        other @ RenderError::Serialize(_) => panic!("expected SelectRequired, got {other:?}"),
    }
}

#[test]
fn all_prints_full_response_unfiltered() {
    let value = json!({"summary": "x", "status": "open", "noise": "kept too"});

    let output = render_json(&value, Select::All).expect("should print everything");
    assert_eq!(output, serde_json::to_string_pretty(&value).expect("serializes"));
}

#[test]
fn or_all_converts_required_to_all() {
    assert!(matches!(Select::Required.or_all(), Select::All));
}

#[test]
fn or_all_leaves_all_unchanged() {
    assert!(matches!(Select::All.or_all(), Select::All));
}

#[test]
fn or_all_leaves_fields_unchanged() {
    let paths = ["summary"];
    match Select::Fields(&paths).or_all() {
        Select::Fields(f) => assert_eq!(f, &paths),
        other => panic!("expected Fields to pass through unchanged, got {other:?}"),
    }
}

#[test]
fn fields_filters_like_filter_fields() {
    let value = json!({"summary": "x", "status": "open", "noise": "dropped"});

    let output =
        render_json(&value, Select::Fields(&["summary", "status"])).expect("should filter");
    assert_eq!(
        output,
        serde_json::to_string_pretty(&json!({"summary": "x", "status": "open"}))
            .expect("serializes")
    );
}
