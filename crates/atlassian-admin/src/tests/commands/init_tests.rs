#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::path::PathBuf;
use tempfile::TempDir;

use super::write_app_config;

fn temp_config_dir() -> (TempDir, PathBuf) {
    let dir = TempDir::new().expect("tempdir");
    let path = dir.path().to_path_buf();
    (dir, path)
}

#[test]
fn write_app_config_creates_file_with_correct_json() {
    let (_dir, config_dir) = temp_config_dir();

    write_app_config(&config_dir, "my-api-key", "my-org-id").expect("should write");

    let app_json_path = config_dir.join("atlassian-admin-cli").join("app.json");
    assert!(app_json_path.exists(), "app.json must exist");

    let content = std::fs::read_to_string(&app_json_path).expect("read");
    let parsed: serde_json::Value = serde_json::from_str(&content).expect("valid JSON");

    assert_eq!(parsed["api_key"], "my-api-key");
    assert_eq!(parsed["org_id"], "my-org-id");
}

#[test]
fn write_app_config_creates_parent_directories() {
    // config_dir may not exist yet on a fresh machine
    let (_dir, config_dir) = temp_config_dir();
    let nested = config_dir.join("does").join("not").join("exist");

    write_app_config(&nested, "key", "org").expect("should create dirs and write");

    let app_json_path = nested.join("atlassian-admin-cli").join("app.json");
    assert!(app_json_path.exists());
}

#[test]
fn write_app_config_overwrites_existing_file() {
    let (_dir, config_dir) = temp_config_dir();

    write_app_config(&config_dir, "old-key", "old-org").expect("first write");
    write_app_config(&config_dir, "new-key", "new-org").expect("second write");

    let app_json_path = config_dir.join("atlassian-admin-cli").join("app.json");
    let content = std::fs::read_to_string(&app_json_path).expect("read");
    let parsed: serde_json::Value = serde_json::from_str(&content).expect("valid JSON");

    assert_eq!(parsed["api_key"], "new-key");
    assert_eq!(parsed["org_id"], "new-org");
}

#[test]
fn write_app_config_written_json_has_only_expected_keys() {
    let (_dir, config_dir) = temp_config_dir();

    write_app_config(&config_dir, "key", "org").expect("write");

    let app_json_path = config_dir.join("atlassian-admin-cli").join("app.json");
    let content = std::fs::read_to_string(&app_json_path).expect("read");
    let parsed: serde_json::Value = serde_json::from_str(&content).expect("valid JSON");
    let obj = parsed.as_object().expect("should be object");

    assert_eq!(obj.len(), 2, "app.json must contain exactly api_key and org_id");
    assert!(obj.contains_key("api_key"));
    assert!(obj.contains_key("org_id"));
}

#[test]
fn write_app_config_accepts_empty_skeleton_values() {
    let (_dir, config_dir) = temp_config_dir();

    write_app_config(&config_dir, "", "").expect("should write skeleton");

    let app_json_path = config_dir.join("atlassian-admin-cli").join("app.json");
    let content = std::fs::read_to_string(&app_json_path).expect("read");
    let parsed: serde_json::Value = serde_json::from_str(&content).expect("valid JSON");

    assert_eq!(parsed["api_key"], "");
    assert_eq!(parsed["org_id"], "");
}
