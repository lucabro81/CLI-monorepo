#![allow(clippy::unwrap_used, clippy::expect_used)]

use super::{build_comment_body, build_create_body, build_merge_body, build_update_body, draft_status, split_reviewers, update_body_from_flags, validate_inline_location, validate_update_has_field};

#[test]
fn build_create_body_with_required_fields_only() {
    let body = build_create_body("My PR", "feature-branch", None, None, false, vec![], false);

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
    let body = build_create_body("My PR", "feature-branch", Some("main".to_string()), None, false, vec![], false);

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
    let body = build_create_body("My PR", "feature-branch", None, Some("does things".to_string()), false, vec![], false);

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
    let body = build_create_body("My PR", "feature-branch", None, None, true, vec![], false);

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
        false,
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
    let body = build_update_body(Some("New title".to_string()), None, None, vec![], None);

    assert_eq!(body, serde_json::json!({"title": "New title"}));
}

#[test]
fn build_update_body_with_description_only() {
    let body = build_update_body(None, Some("New description".to_string()), None, vec![], None);

    assert_eq!(body, serde_json::json!({"description": "New description"}));
}

#[test]
fn build_update_body_with_destination_only() {
    let body = build_update_body(None, None, Some("develop".to_string()), vec![], None);

    assert_eq!(body, serde_json::json!({"destination": {"branch": {"name": "develop"}}}));
}

#[test]
fn build_update_body_with_reviewers_only() {
    let body = build_update_body(None, None, None, vec!["{uuid-1}".to_string(), "{uuid-2}".to_string()], None);

    assert_eq!(body, serde_json::json!({"reviewers": [{"uuid": "{uuid-1}"}, {"uuid": "{uuid-2}"}]}));
}

#[test]
fn build_update_body_with_all_fields() {
    let body = build_update_body(
        Some("New title".to_string()),
        Some("New description".to_string()),
        Some("develop".to_string()),
        vec!["{uuid-1}".to_string()],
        Some(true),
    );

    assert_eq!(
        body,
        serde_json::json!({
            "title": "New title",
            "description": "New description",
            "destination": {"branch": {"name": "develop"}},
            "reviewers": [{"uuid": "{uuid-1}"}],
            "draft": true
        })
    );
}

#[test]
fn build_update_body_with_draft_true_only() {
    let body = build_update_body(None, None, None, vec![], Some(true));

    assert_eq!(body, serde_json::json!({"draft": true}));
}

// Guards against dropping `draft: false` from the body (e.g. treating it like an unset
// bool flag): `--ready-for-review` relies on sending an explicit `false` to publish a draft.
#[test]
fn build_update_body_with_draft_false_only() {
    let body = build_update_body(None, None, None, vec![], Some(false));

    assert_eq!(body, serde_json::json!({"draft": false}));
}

#[test]
fn build_update_body_with_no_fields_is_empty_object() {
    let body = build_update_body(None, None, None, vec![], None);

    assert_eq!(body, serde_json::json!({}));
}

#[test]
fn validate_update_has_field_errs_when_all_absent() {
    let err = validate_update_has_field(None, None, None, &[], None).expect_err("should error");

    match err {
        crate::error::CliError::InvalidInput { reason } => {
            assert!(reason.contains("--draft"), "error should list --draft: {reason}");
            assert!(reason.contains("--ready-for-review"), "error should list --ready-for-review: {reason}");
        }
        other => panic!("expected InvalidInput, got {other:?}"),
    }
}

#[test]
fn validate_update_has_field_ok_when_only_draft_true_set() {
    let result = validate_update_has_field(None, None, None, &[], Some(true));

    assert!(result.is_ok());
}

#[test]
fn validate_update_has_field_ok_when_only_draft_false_set() {
    let result = validate_update_has_field(None, None, None, &[], Some(false));

    assert!(result.is_ok());
}

#[test]
fn validate_update_has_field_ok_when_only_title_set() {
    let result = validate_update_has_field(Some("New title"), None, None, &[], None);

    assert!(result.is_ok());
}

#[test]
fn validate_update_has_field_ok_when_only_reviewers_set() {
    let result = validate_update_has_field(None, None, None, &["{uuid-1}".to_string()], None);

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
    let body = build_comment_body("Looks good to me", None, None);

    assert_eq!(
        body,
        serde_json::json!({
            "content": {"raw": "Looks good to me"}
        })
    );
}

#[test]
fn build_comment_body_inline_comment() {
    let body = build_comment_body("Fix this", Some(("src/main.rs".to_string(), 10)), None);

    assert_eq!(
        body,
        serde_json::json!({
            "content": {"raw": "Fix this"},
            "inline": {"path": "src/main.rs", "to": 10}
        })
    );
}

#[test]
fn build_comment_body_reply() {
    let body = build_comment_body("Done, fixed", None, Some(123_456));

    assert_eq!(
        body,
        serde_json::json!({
            "content": {"raw": "Done, fixed"},
            "parent": {"id": 123_456}
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
        true,
    );

    assert_eq!(
        body,
        serde_json::json!({
            "title": "My PR",
            "source": {"branch": {"name": "feature-branch"}},
            "destination": {"branch": {"name": "main"}},
            "description": "does things",
            "close_source_branch": true,
            "reviewers": [{"uuid": "{uuid-1}"}],
            "draft": true
        })
    );
}

#[test]
fn build_create_body_includes_draft_when_true() {
    let body = build_create_body("My PR", "feature-branch", None, None, false, vec![], true);

    assert_eq!(
        body,
        serde_json::json!({
            "title": "My PR",
            "source": {"branch": {"name": "feature-branch"}},
            "draft": true
        })
    );
}

#[test]
fn draft_status_is_none_when_neither_flag_set() {
    assert_eq!(draft_status(false, false), None);
}

#[test]
fn draft_status_is_true_for_draft_flag() {
    assert_eq!(draft_status(true, false), Some(true));
}

#[test]
fn draft_status_is_false_for_ready_for_review_flag() {
    assert_eq!(draft_status(false, true), Some(false));
}

#[test]
fn update_body_from_flags_ready_for_review_only_sends_draft_false() {
    let body = update_body_from_flags(None, None, None, None, false, true).expect("should build");

    assert_eq!(body, serde_json::json!({"draft": false}));
}

#[test]
fn update_body_from_flags_combines_draft_with_other_fields() {
    let body = update_body_from_flags(Some("New title".to_string()), None, None, Some("{uuid-1}"), true, false)
        .expect("should build");

    assert_eq!(
        body,
        serde_json::json!({"title": "New title", "reviewers": [{"uuid": "{uuid-1}"}], "draft": true})
    );
}

#[test]
fn update_body_from_flags_errs_when_no_flags_set() {
    let err = update_body_from_flags(None, None, None, None, false, false).expect_err("should error");

    assert!(matches!(err, crate::error::CliError::InvalidInput { .. }));
}

#[test]
fn update_body_from_flags_empty_reviewers_with_ready_for_review_is_valid() {
    let body = update_body_from_flags(None, None, None, Some(""), false, true).expect("should build");

    assert_eq!(body, serde_json::json!({"draft": false}));
}
