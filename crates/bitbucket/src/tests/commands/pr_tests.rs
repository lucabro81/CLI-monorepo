#![allow(clippy::unwrap_used, clippy::expect_used)]

use super::{build_comment_body, build_create_body, build_merge_body, build_update_body, split_reviewers, validate_inline_location, validate_update_has_field};

#[test]
fn build_create_body_with_required_fields_only() {
    let body = build_create_body("My PR", "feature-branch", None, None, false, vec![]);

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
    let body = build_create_body("My PR", "feature-branch", Some("main".to_string()), None, false, vec![]);

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
    let body = build_create_body("My PR", "feature-branch", None, Some("does things".to_string()), false, vec![]);

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
    let body = build_create_body("My PR", "feature-branch", None, None, true, vec![]);

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
fn build_create_body_includes_reviewers_when_set() {
    let body = build_create_body(
        "My PR",
        "feature-branch",
        None,
        None,
        false,
        vec!["{uuid-1}".to_string(), "{uuid-2}".to_string()],
    );

    assert_eq!(
        body,
        serde_json::json!({
            "title": "My PR",
            "source": {"branch": {"name": "feature-branch"}},
            "reviewers": [{"uuid": "{uuid-1}"}, {"uuid": "{uuid-2}"}]
        })
    );
}

#[test]
fn build_update_body_with_title_only() {
    let body = build_update_body(Some("New title".to_string()), None, None, vec![]);

    assert_eq!(body, serde_json::json!({"title": "New title"}));
}

#[test]
fn build_update_body_with_description_only() {
    let body = build_update_body(None, Some("New description".to_string()), None, vec![]);

    assert_eq!(body, serde_json::json!({"description": "New description"}));
}

#[test]
fn build_update_body_with_destination_only() {
    let body = build_update_body(None, None, Some("develop".to_string()), vec![]);

    assert_eq!(body, serde_json::json!({"destination": {"branch": {"name": "develop"}}}));
}

#[test]
fn build_update_body_with_reviewers_only() {
    let body = build_update_body(None, None, None, vec!["{uuid-1}".to_string(), "{uuid-2}".to_string()]);

    assert_eq!(body, serde_json::json!({"reviewers": [{"uuid": "{uuid-1}"}, {"uuid": "{uuid-2}"}]}));
}

#[test]
fn build_update_body_with_all_fields() {
    let body = build_update_body(
        Some("New title".to_string()),
        Some("New description".to_string()),
        Some("develop".to_string()),
        vec!["{uuid-1}".to_string()],
    );

    assert_eq!(
        body,
        serde_json::json!({
            "title": "New title",
            "description": "New description",
            "destination": {"branch": {"name": "develop"}},
            "reviewers": [{"uuid": "{uuid-1}"}]
        })
    );
}

#[test]
fn build_update_body_with_no_fields_is_empty_object() {
    let body = build_update_body(None, None, None, vec![]);

    assert_eq!(body, serde_json::json!({}));
}

#[test]
fn validate_update_has_field_errs_when_all_absent() {
    let result = validate_update_has_field(None, None, None, &[]);

    assert!(result.is_err());
}

#[test]
fn validate_update_has_field_ok_when_only_title_set() {
    let result = validate_update_has_field(Some("New title"), None, None, &[]);

    assert!(result.is_ok());
}

#[test]
fn validate_update_has_field_ok_when_only_reviewers_set() {
    let result = validate_update_has_field(None, None, None, &["{uuid-1}".to_string()]);

    assert!(result.is_ok());
}

#[test]
fn split_reviewers_returns_empty_vec_when_none() {
    assert_eq!(split_reviewers(None), Vec::<String>::new());
}

#[test]
fn split_reviewers_trims_and_filters_empty_entries() {
    assert_eq!(
        split_reviewers(Some(" {uuid-1} , {uuid-2},  ")),
        vec!["{uuid-1}".to_string(), "{uuid-2}".to_string()]
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
fn build_merge_body_with_no_optional_fields() {
    let body = build_merge_body(None, None, false);

    assert_eq!(body, serde_json::json!({}));
}

#[test]
fn build_merge_body_includes_message_when_set() {
    let body = build_merge_body(Some("Merging feature".to_string()), None, false);

    assert_eq!(body, serde_json::json!({"message": "Merging feature"}));
}

#[test]
fn build_merge_body_includes_merge_strategy_when_set() {
    let body = build_merge_body(None, Some("squash".to_string()), false);

    assert_eq!(body, serde_json::json!({"merge_strategy": "squash"}));
}

#[test]
fn build_merge_body_includes_close_source_branch_when_true() {
    let body = build_merge_body(None, None, true);

    assert_eq!(body, serde_json::json!({"close_source_branch": true}));
}

#[test]
fn build_merge_body_combines_all_fields() {
    let body = build_merge_body(Some("Merging feature".to_string()), Some("squash".to_string()), true);

    assert_eq!(
        body,
        serde_json::json!({
            "message": "Merging feature",
            "merge_strategy": "squash",
            "close_source_branch": true
        })
    );
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
        vec!["{uuid-1}".to_string()],
    );

    assert_eq!(
        body,
        serde_json::json!({
            "title": "My PR",
            "source": {"branch": {"name": "feature-branch"}},
            "destination": {"branch": {"name": "main"}},
            "description": "does things",
            "close_source_branch": true,
            "reviewers": [{"uuid": "{uuid-1}"}]
        })
    );
}
