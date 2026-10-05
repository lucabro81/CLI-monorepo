#![allow(clippy::unwrap_used, clippy::expect_used)]

use serde_json::json;

use super::build_list_body;

#[test]
fn no_filter_sends_only_pagination() {
    // The v2 ProjectService uses the newer "pagination"/"filters" request shape,
    // not the "query"/"queries" shape of the user and organization services.
    assert_eq!(
        build_list_body(None, None, 100, 0),
        json!({"pagination": {"offset": 0, "limit": 100, "asc": true}, "filters": []})
    );
}

#[test]
fn name_and_organization_filters_are_combined() {
    assert_eq!(
        build_list_body(Some("app"), Some("org-1"), 5, 10),
        json!({
            "pagination": {"offset": 10, "limit": 5, "asc": true},
            "filters": [
                {"projectNameFilter": {"projectName": "app", "method": "TEXT_FILTER_METHOD_CONTAINS_IGNORE_CASE"}},
                {"organizationIdFilter": {"organizationId": "org-1"}}
            ]
        })
    );
}
