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

#[cfg(test)]
#[path = "fields_tests.rs"]
mod tests;
