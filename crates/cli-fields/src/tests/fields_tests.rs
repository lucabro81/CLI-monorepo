#![allow(clippy::unwrap_used, clippy::expect_used)]

use serde_json::json;

use super::{describe_top_level_shape, filter_fields};

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

// --- Edge cases (BACKLOG FIELDS-1, FIELDS-2, FIELDS-3, FIELDS-4) ---

#[test]
fn empty_string_field_path_produces_empty_object() {
    // BACKLOG FIELDS-1: split(',') on a trailing comma yields ""; documents current behaviour.
    let value = json!({"summary": "x"});

    assert_eq!(filter_fields(value, &[""]), json!({}));
}

#[test]
fn all_fields_missing_returns_empty_object() {
    // BACKLOG FIELDS-2: caller gets {} with no error when all paths are wrong.
    let value = json!({"summary": "x", "status": "open"});

    assert_eq!(filter_fields(value, &["nonexistent", "also.missing"]), json!({}));
}

#[test]
fn nested_path_where_intermediate_is_null_returns_null() {
    // BACKLOG FIELDS-3: status is null; .name segment hits other => clone arm.
    let value = json!({"status": null, "noise": true});

    assert_eq!(
        filter_fields(value, &["status.name"]),
        json!({"status": null})
    );
}

#[test]
fn nested_path_where_intermediate_is_scalar_returns_scalar() {
    // BACKLOG FIELDS-4: status is a string, not an object; .name is silently ignored.
    let value = json!({"status": "open", "noise": true});

    assert_eq!(
        filter_fields(value, &["status.name"]),
        json!({"status": "open"})
    );
}

#[test]
fn duplicate_fields_no_crash_and_correct_output() {
    let value = json!({"summary": "Fix bug", "noise": true});

    assert_eq!(
        filter_fields(value, &["summary", "summary"]),
        json!({"summary": "Fix bug"})
    );
}

#[test]
fn empty_array_returns_empty_array() {
    let value = json!([]);

    assert_eq!(filter_fields(value, &["name", "id"]), json!([]));
}

// --- describe_top_level_shape ---

#[test]
fn describes_object_top_level_fields() {
    let value = json!({"summary": "x", "status": "open", "assignee": null});

    // serde_json::Map is BTreeMap-backed here (no preserve_order feature) → alphabetical.
    assert_eq!(
        describe_top_level_shape(&value),
        "top-level fields: assignee, status, summary"
    );
}

#[test]
fn describes_empty_object() {
    assert_eq!(
        describe_top_level_shape(&json!({})),
        "the response is an empty top-level object (no fields)"
    );
}

#[test]
fn describes_array_of_objects_with_count_and_first_element_fields() {
    let value = json!([{"id": "1", "name": "a"}, {"id": "2", "name": "b"}]);

    assert_eq!(
        describe_top_level_shape(&value),
        "top-level array with 2 element(s); first element's fields: id, name"
    );
}

#[test]
fn describes_array_of_scalars_with_count_only() {
    let value = json!(["a", "b", "c"]);

    assert_eq!(
        describe_top_level_shape(&value),
        "top-level array with 3 element(s)"
    );
}

#[test]
fn describes_empty_array() {
    assert_eq!(
        describe_top_level_shape(&json!([])),
        "the response is an empty top-level array (0 elements, no fields to select)"
    );
}

#[test]
fn describes_scalar_top_level_value() {
    assert_eq!(
        describe_top_level_shape(&json!("just a string")),
        "the response is a top-level string value, not an object or array"
    );
}

#[test]
fn describes_null_top_level_value() {
    assert_eq!(
        describe_top_level_shape(&serde_json::Value::Null),
        "the response is a top-level null value, not an object or array"
    );
}
