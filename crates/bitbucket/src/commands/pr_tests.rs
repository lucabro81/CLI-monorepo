#![allow(clippy::unwrap_used, clippy::expect_used)]

use super::{build_comment_body, build_create_body, validate_inline_location};

#[test]
fn build_create_body_with_required_fields_only() {
    let body = build_create_body("My PR", "feature-branch", None, None, false);

    assert_eq!(
        body,
        serde_json::json!({
            "title": "My PR",
            "source": {"branch": {"name": "feature-branch"}}
        })
    );
}

#[test]
fn build_create_body_includes_destination_when_set() {
    let body = build_create_body("My PR", "feature-branch", Some("main".to_string()), None, false);

    assert_eq!(
        body,
        serde_json::json!({
            "title": "My PR",
            "source": {"branch": {"name": "feature-branch"}},
            "destination": {"branch": {"name": "main"}}
        })
    );
}

#[test]
fn build_create_body_includes_description_when_set() {
    let body = build_create_body("My PR", "feature-branch", None, Some("does things".to_string()), false);

    assert_eq!(
        body,
        serde_json::json!({
            "title": "My PR",
            "source": {"branch": {"name": "feature-branch"}},
            "description": "does things"
        })
    );
}

#[test]
fn build_create_body_includes_close_source_branch_when_true() {
    let body = build_create_body("My PR", "feature-branch", None, None, true);

    assert_eq!(
        body,
        serde_json::json!({
            "title": "My PR",
            "source": {"branch": {"name": "feature-branch"}},
            "close_source_branch": true
        })
    );
}

#[test]
fn validate_inline_location_returns_none_when_both_absent() {
    let location = validate_inline_location(None, None).expect("should validate");

    assert_eq!(location, None);
}

#[test]
fn validate_inline_location_returns_some_when_both_present() {
    let location = validate_inline_location(Some("src/main.rs".to_string()), Some(10)).expect("should validate");

    assert_eq!(location, Some(("src/main.rs".to_string(), 10)));
}

#[test]
fn validate_inline_location_errors_when_only_path_present() {
    let err = validate_inline_location(Some("src/main.rs".to_string()), None).expect_err("should error");

    assert!(matches!(err, crate::error::CliError::InvalidInput { .. }));
}

#[test]
fn validate_inline_location_errors_when_only_line_present() {
    let err = validate_inline_location(None, Some(10)).expect_err("should error");

    assert!(matches!(err, crate::error::CliError::InvalidInput { .. }));
}

#[test]
fn build_comment_body_general_comment() {
    let body = build_comment_body("Looks good to me", None);

    assert_eq!(
        body,
        serde_json::json!({
            "content": {"raw": "Looks good to me"}
        })
    );
}

#[test]
fn build_comment_body_inline_comment() {
    let body = build_comment_body("Fix this", Some(("src/main.rs".to_string(), 10)));

    assert_eq!(
        body,
        serde_json::json!({
            "content": {"raw": "Fix this"},
            "inline": {"path": "src/main.rs", "to": 10}
        })
    );
}

#[test]
fn build_create_body_combines_all_fields() {
    let body = build_create_body(
        "My PR",
        "feature-branch",
        Some("main".to_string()),
        Some("does things".to_string()),
        true,
    );

    assert_eq!(
        body,
        serde_json::json!({
            "title": "My PR",
            "source": {"branch": {"name": "feature-branch"}},
            "destination": {"branch": {"name": "main"}},
            "description": "does things",
            "close_source_branch": true
        })
    );
}
