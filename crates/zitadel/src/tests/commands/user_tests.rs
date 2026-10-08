#![allow(clippy::unwrap_used, clippy::expect_used)]

use serde_json::json;

use super::{UserSearchFilters, build_authorizations_body, build_search_body};
use crate::cli::{AuthorizationState, UserState};

fn filters() -> UserSearchFilters<'static> {
    UserSearchFilters {
        email: None,
        username: None,
        state: None,
        organization_id: None,
        limit: 100,
        offset: 0,
    }
}

#[test]
fn no_filters_sends_only_pagination() {
    assert_eq!(
        build_search_body(&filters()),
        json!({"query": {"offset": 0, "limit": 100, "asc": true}, "queries": []})
    );
}

#[test]
fn text_filters_use_contains_ignore_case() {
    let body = build_search_body(&UserSearchFilters {
        email: Some("@acme.com"),
        username: Some("john"),
        ..filters()
    });

    assert_eq!(
        body["queries"],
        json!([
            {"emailQuery": {"emailAddress": "@acme.com", "method": "TEXT_QUERY_METHOD_CONTAINS_IGNORE_CASE"}},
            {"userNameQuery": {"userName": "john", "method": "TEXT_QUERY_METHOD_CONTAINS_IGNORE_CASE"}}
        ])
    );
}

#[test]
fn all_filters_are_combined_and_pagination_is_passed_through() {
    let body = build_search_body(&UserSearchFilters {
        email: Some("a@b.c"),
        username: Some("u"),
        state: Some(UserState::Locked),
        organization_id: Some("org-1"),
        limit: 10,
        offset: 20,
    });

    assert_eq!(
        body,
        json!({
            "query": {"offset": 20, "limit": 10, "asc": true},
            "queries": [
                {"emailQuery": {"emailAddress": "a@b.c", "method": "TEXT_QUERY_METHOD_CONTAINS_IGNORE_CASE"}},
                {"userNameQuery": {"userName": "u", "method": "TEXT_QUERY_METHOD_CONTAINS_IGNORE_CASE"}},
                {"stateQuery": {"state": "USER_STATE_LOCKED"}},
                {"organizationIdQuery": {"organizationId": "org-1"}}
            ]
        })
    );
}

#[test]
fn every_state_maps_to_its_zitadel_v2_enum_value() {
    // ZITADEL answers an unknown enum value with 200 and zero results instead of
    // an error, so a wrong mapping would silently look like "no users".
    let cases = [
        (UserState::Active, "USER_STATE_ACTIVE"),
        (UserState::Inactive, "USER_STATE_INACTIVE"),
        (UserState::Deleted, "USER_STATE_DELETED"),
        (UserState::Locked, "USER_STATE_LOCKED"),
        (UserState::Initial, "USER_STATE_INITIAL"),
    ];
    for (state, expected) in cases {
        let body = build_search_body(&UserSearchFilters { state: Some(state), ..filters() });
        assert_eq!(body["queries"][0]["stateQuery"]["state"], expected, "{state:?}");
    }
}

// ── user authorizations (issue #229) ──────────────────────────────────────

#[test]
fn authorizations_body_filters_by_the_user() {
    assert_eq!(
        build_authorizations_body("123", None, None, 100, 0),
        json!({
            "pagination": {"offset": 0, "limit": 100, "asc": true},
            "filters": [{"inUserIds": {"ids": ["123"]}}],
        })
    );
}

#[test]
fn authorizations_body_narrows_by_project_and_state() {
    assert_eq!(
        build_authorizations_body("123", Some("456"), Some(AuthorizationState::Active), 5, 10),
        json!({
            "pagination": {"offset": 10, "limit": 5, "asc": true},
            "filters": [
                {"inUserIds": {"ids": ["123"]}},
                {"projectId": {"id": "456"}},
                {"state": {"state": "STATE_ACTIVE"}},
            ],
        })
    );
    assert_eq!(
        build_authorizations_body("123", None, Some(AuthorizationState::Inactive), 100, 0)["filters"][1],
        json!({"state": {"state": "STATE_INACTIVE"}})
    );
}

