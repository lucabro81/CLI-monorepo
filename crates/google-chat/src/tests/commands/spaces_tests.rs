#![allow(clippy::unwrap_used, clippy::expect_used)]

use super::build_members_response;

#[test]
fn build_members_response_resolves_human_members() {
    let memberships = serde_json::json!({
        "memberships": [
            {"member": {"name": "users/1", "type": "HUMAN"}},
            {"member": {"name": "users/2", "type": "HUMAN"}},
        ],
        "nextPageToken": "abc123",
    });

    let result = build_members_response(&memberships, |name| {
        Ok(serde_json::json!({"resourceName": format!("people/{}", &name[6..]), "names": [{"displayName": name}]}))
    });

    assert_eq!(
        result,
        serde_json::json!({
            "members": [
                {"resourceName": "people/1", "names": [{"displayName": "users/1"}]},
                {"resourceName": "people/2", "names": [{"displayName": "users/2"}]},
            ],
            "unresolved": [],
            "nextPageToken": "abc123",
        })
    );
}

#[test]
fn build_members_response_skips_non_human_members_without_calling_resolve() {
    let memberships = serde_json::json!({
        "memberships": [
            {"member": {"name": "users/bot1", "type": "BOT"}},
        ],
    });

    let result = build_members_response(&memberships, |_name| {
        panic!("resolve must not be called for non-HUMAN members")
    });

    assert_eq!(
        result,
        serde_json::json!({
            "members": [],
            "unresolved": [
                {
                    "member": "users/bot1",
                    "reason": "member type is not HUMAN; the People API only resolves human Google accounts",
                }
            ],
        })
    );
}

#[test]
fn build_members_response_collects_resolution_failures_without_failing_the_rest() {
    let memberships = serde_json::json!({
        "memberships": [
            {"member": {"name": "users/1", "type": "HUMAN"}},
            {"member": {"name": "users/2", "type": "HUMAN"}},
        ],
    });

    let result = build_members_response(&memberships, |name| {
        if name == "users/1" {
            Err("People API returned status 403: cross-domain user".to_string())
        } else {
            Ok(serde_json::json!({"resourceName": "people/2"}))
        }
    });

    assert_eq!(
        result,
        serde_json::json!({
            "members": [
                {"resourceName": "people/2"},
            ],
            "unresolved": [
                {"member": "users/1", "reason": "People API returned status 403: cross-domain user"},
            ],
        })
    );
}

#[test]
fn build_members_response_handles_empty_memberships() {
    let memberships = serde_json::json!({"memberships": []});

    let result = build_members_response(&memberships, |_name| {
        panic!("resolve must not be called when there are no memberships")
    });

    assert_eq!(
        result,
        serde_json::json!({"members": [], "unresolved": []})
    );
}

#[test]
fn build_members_response_omits_next_page_token_when_absent() {
    let memberships = serde_json::json!({"memberships": []});

    let result = build_members_response(&memberships, |_name| {
        panic!("resolve must not be called when there are no memberships")
    });

    assert!(result.get("nextPageToken").is_none());
}
