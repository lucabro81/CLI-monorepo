#![allow(clippy::unwrap_used, clippy::expect_used)]

use serde_json::json;

use super::filter_fields;

#[test]
fn no_fields_returns_value_unchanged() {
    let value = json!({"a": 1, "b": 2});

    assert_eq!(filter_fields(value.clone(), &[]), value);
}

#[test]
fn extracts_top_level_field() {
    let value = json!({"summary": "Fix bug", "status": "open", "noise": "ignored"});

    assert_eq!(
        filter_fields(value, &["summary", "status"]),
        json!({"summary": "Fix bug", "status": "open"})
    );
}

#[test]
fn extracts_nested_field_via_dot_notation() {
    let value = json!({"status": {"name": "In Progress", "id": "3"}, "noise": true});

    assert_eq!(
        filter_fields(value, &["status.name"]),
        json!({"status": {"name": "In Progress"}})
    );
}

#[test]
fn applies_filter_element_wise_on_array() {
    let value = json!([
        {"name": "To Do", "id": "11", "noise": true},
        {"name": "Done", "id": "41", "noise": true}
    ]);

    assert_eq!(
        filter_fields(value, &["name", "id"]),
        json!([{"name": "To Do", "id": "11"}, {"name": "Done", "id": "41"}])
    );
}

#[test]
fn applies_filter_to_nested_array() {
    // Simulates the transitions endpoint: {transitions: [{id, name, hasScreen, ...}]}
    let value = json!({
        "expand": "transitions",
        "transitions": [
            {"id": "11", "name": "To Do", "hasScreen": false},
            {"id": "21", "name": "In Progress", "hasScreen": false}
        ]
    });

    assert_eq!(
        filter_fields(value, &["transitions.id", "transitions.name"]),
        json!({
            "transitions": [
                {"id": "11", "name": "To Do"},
                {"id": "21", "name": "In Progress"}
            ]
        })
    );
}

#[test]
fn missing_field_omitted_from_result() {
    let value = json!({"a": 1});

    assert_eq!(
        filter_fields(value, &["a", "nonexistent"]),
        json!({"a": 1})
    );
}

#[test]
fn extracts_top_level_object_field_in_full() {
    // Selecting a key whose value is an object without further dot-path → whole object included
    let value = json!({"status": {"name": "Done", "id": "3"}, "summary": "x"});

    assert_eq!(
        filter_fields(value, &["status"]),
        json!({"status": {"name": "Done", "id": "3"}})
    );
}

#[test]
fn handles_deeply_nested_path() {
    let value = json!({"a": {"b": {"c": 42, "d": 99}}});

    assert_eq!(
        filter_fields(value, &["a.b.c"]),
        json!({"a": {"b": {"c": 42}}})
    );
}

#[test]
fn multiple_paths_share_prefix_merged() {
    let value = json!({"status": {"name": "Done", "id": "3", "noise": true}});

    assert_eq!(
        filter_fields(value, &["status.name", "status.id"]),
        json!({"status": {"name": "Done", "id": "3"}})
    );
}
