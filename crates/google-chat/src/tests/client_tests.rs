#![allow(clippy::unwrap_used, clippy::expect_used)]

use super::{build_setup_space_body, normalize_space_name, normalize_user_name};

#[test]
fn normalize_space_name_passes_through_full_resource_name() {
    assert_eq!(normalize_space_name("spaces/AAQA-_d58OQ"), "spaces/AAQA-_d58OQ");
}

#[test]
fn normalize_space_name_prefixes_bare_id() {
    assert_eq!(normalize_space_name("AAQA-_d58OQ"), "spaces/AAQA-_d58OQ");
}

#[test]
fn normalize_space_name_does_not_double_prefix() {
    // A bare id that happens to start with "spaces" but isn't the "spaces/" resource
    // prefix must still get a single "spaces/" added, not be mistaken for already-prefixed.
    assert_eq!(normalize_space_name("spacesfoo"), "spaces/spacesfoo");
}

#[test]
fn normalize_user_name_passes_through_full_resource_name() {
    assert_eq!(normalize_user_name("users/108506379394699518479"), "users/108506379394699518479");
}

#[test]
fn normalize_user_name_prefixes_bare_email() {
    assert_eq!(normalize_user_name("colleague@example.com"), "users/colleague@example.com");
}

#[test]
fn normalize_user_name_prefixes_bare_id() {
    assert_eq!(normalize_user_name("108506379394699518479"), "users/108506379394699518479");
}

#[test]
fn build_setup_space_body_with_one_user_is_direct_message() {
    let body = build_setup_space_body(&["colleague@example.com".to_string()]);

    assert_eq!(
        body,
        serde_json::json!({
            "space": { "spaceType": "DIRECT_MESSAGE" },
            "memberships": [
                { "member": { "name": "users/colleague@example.com", "type": "HUMAN" } },
            ],
        })
    );
}

#[test]
fn build_setup_space_body_with_multiple_users_is_group_chat() {
    let body = build_setup_space_body(&["colleague@example.com".to_string(), "other@example.com".to_string()]);

    assert_eq!(
        body,
        serde_json::json!({
            "space": { "spaceType": "GROUP_CHAT" },
            "memberships": [
                { "member": { "name": "users/colleague@example.com", "type": "HUMAN" } },
                { "member": { "name": "users/other@example.com", "type": "HUMAN" } },
            ],
        })
    );
}

#[test]
fn build_setup_space_body_normalizes_already_prefixed_user() {
    let body = build_setup_space_body(&["users/108506379394699518479".to_string()]);

    assert_eq!(
        body["memberships"][0]["member"]["name"],
        serde_json::json!("users/108506379394699518479")
    );
}
