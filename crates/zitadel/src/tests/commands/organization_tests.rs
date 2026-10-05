#![allow(clippy::unwrap_used, clippy::expect_used)]

use serde_json::json;

use super::build_list_body;

#[test]
fn no_filter_sends_only_pagination() {
    assert_eq!(
        build_list_body(None, 100, 0),
        json!({"query": {"offset": 0, "limit": 100, "asc": true}, "queries": []})
    );
}

#[test]
fn name_filter_uses_contains_ignore_case() {
    assert_eq!(
        build_list_body(Some("acme"), 5, 10),
        json!({
            "query": {"offset": 10, "limit": 5, "asc": true},
            "queries": [{"nameQuery": {"name": "acme", "method": "TEXT_QUERY_METHOD_CONTAINS_IGNORE_CASE"}}]
        })
    );
}
