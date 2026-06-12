#![allow(clippy::unwrap_used, clippy::expect_used)]

use super::build_create_body;

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
