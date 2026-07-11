//! Client-side JSON field projection via dot-notation paths.
//!
//! `filter_fields` is the implementation of the `--select` global flag. It
//! accepts a `serde_json::Value` and a list of dot-notation paths (e.g.
//! `["status.name", "assignee.displayName"]`) and returns a new value
//! containing only the requested fields.
//!
//! Arrays are handled element-wise automatically: `"transitions.name"` on a
//! response where `transitions` is an array will extract the `name` field
//! from each element without requiring any special syntax from the caller.
//!
//! Internally, paths are compiled into a `FieldTree` (a recursive
//! `BTreeMap`) so that sibling paths sharing a common prefix (e.g.
//! `"status.id"` and `"status.name"`) are resolved with a single object
//! traversal rather than repeated passes over the value.

use std::collections::BTreeMap;

use serde_json::{Map, Value};

/// Filters a JSON value to include only the specified dot-notation field paths.
///
/// - Top-level fields: `"summary"`, `"status"`
/// - Nested fields: `"status.name"`, `"assignee.displayName"`
/// - If the value at any step is an array, filtering is applied element-wise.
/// - If `fields` is empty, the value is returned unchanged.
/// - Missing fields are silently omitted from the result.
pub fn filter_fields(value: Value, fields: &[&str]) -> Value {
    if fields.is_empty() {
        return value;
    }
    let tree = FieldTree::build(fields);
    apply_tree(&value, &tree)
}

/// A selection tree derived from dot-notation paths.
/// Each node maps a key to its subtree; a leaf has an empty map.
#[derive(Default)]
struct FieldTree(BTreeMap<String, FieldTree>);

impl FieldTree {
    fn build(fields: &[&str]) -> Self {
        let mut tree = FieldTree::default();
        for &field in fields {
            tree.insert_path(field);
        }
        tree
    }

    fn insert_path(&mut self, path: &str) {
        let (head, tail) = match path.split_once('.') {
            Some((h, t)) => (h, Some(t)),
            None => (path, None),
        };
        let subtree = self.0.entry(head.to_string()).or_default();
        if let Some(rest) = tail {
            subtree.insert_path(rest);
        }
    }

    fn is_leaf(&self) -> bool {
        self.0.is_empty()
    }
}

fn apply_tree(value: &Value, tree: &FieldTree) -> Value {
    match value {
        Value::Array(arr) => Value::Array(arr.iter().map(|v| apply_tree(v, tree)).collect()),
        Value::Object(obj) => {
            let mut result = Map::new();
            for (key, subtree) in &tree.0 {
                if let Some(child) = obj.get(key) {
                    if subtree.is_leaf() {
                        result.insert(key.clone(), child.clone());
                    } else {
                        result.insert(key.clone(), apply_tree(child, subtree));
                    }
                }
            }
            Value::Object(result)
        }
        other => other.clone(),
    }
}

/// Describes the top-level shape of a JSON value for the `--select`-required
/// error: what a caller has available to build a `--select` path from. Not a
/// general introspection tool — only covers the shapes this CLI's API
/// responses actually take (a top-level object, or a top-level array of
/// objects for list endpoints).
pub fn describe_top_level_shape(value: &Value) -> String {
    match value {
        Value::Object(map) if map.is_empty() => {
            "the response is an empty top-level object (no fields)".to_string()
        }
        Value::Object(map) => {
            let keys = map.keys().cloned().collect::<Vec<_>>().join(", ");
            format!("top-level fields: {keys}")
        }
        Value::Array(arr) if arr.is_empty() => {
            "the response is an empty top-level array (0 elements, no fields to select)"
                .to_string()
        }
        Value::Array(arr) => match arr[0].as_object() {
            Some(first) if !first.is_empty() => {
                let keys = first.keys().cloned().collect::<Vec<_>>().join(", ");
                format!(
                    "top-level array with {} element(s); first element's fields: {keys}",
                    arr.len()
                )
            }
            _ => format!("top-level array with {} element(s)", arr.len()),
        },
        other => format!(
            "the response is a top-level {} value, not an object or array",
            scalar_type_name(other)
        ),
    }
}

fn scalar_type_name(value: &Value) -> &'static str {
    match value {
        Value::Null => "null",
        Value::Bool(_) => "boolean",
        Value::Number(_) => "number",
        Value::String(_) => "string",
        Value::Array(_) | Value::Object(_) => "value", // unreachable from describe_top_level_shape
    }
}

#[cfg(test)]
#[path = "tests/fields_tests.rs"]
mod tests;
