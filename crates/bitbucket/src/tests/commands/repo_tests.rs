#![allow(clippy::unwrap_used, clippy::expect_used)]

use super::build_create_body;

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
