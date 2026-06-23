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

    write_app_config(&config_dir, "my-client-id", "my-client-secret").expect("should write");

    let app_json_path = config_dir.join("google-chat-cli").join("app.json");
    assert!(app_json_path.exists(), "app.json must exist");

    let content = std::fs::read_to_string(&app_json_path).expect("read");
    let parsed: serde_json::Value = serde_json::from_str(&content).expect("valid JSON");

    assert_eq!(parsed["client_id"], "my-client-id");
    assert_eq!(parsed["client_secret"], "my-client-secret");
}

#[test]
fn write_app_config_creates_parent_directories() {
    // config_dir may not exist yet on a fresh machine
    let (_dir, config_dir) = temp_config_dir();
    let nested = config_dir.join("does").join("not").join("exist");

    write_app_config(&nested, "id", "secret").expect("should create dirs and write");

    let app_json_path = nested.join("google-chat-cli").join("app.json");
    assert!(app_json_path.exists());
}

#[test]
fn write_app_config_overwrites_client_credentials() {
    let (_dir, config_dir) = temp_config_dir();

    write_app_config(&config_dir, "old-id", "old-secret").expect("first write");
    write_app_config(&config_dir, "new-id", "new-secret").expect("second write");

    let app_json_path = config_dir.join("google-chat-cli").join("app.json");
    let content = std::fs::read_to_string(&app_json_path).expect("read");
    let parsed: serde_json::Value = serde_json::from_str(&content).expect("valid JSON");

    assert_eq!(parsed["client_id"], "new-id");
    assert_eq!(parsed["client_secret"], "new-secret");
}

#[test]
fn write_app_config_without_prior_file_has_only_client_keys() {
    let (_dir, config_dir) = temp_config_dir();

    write_app_config(&config_dir, "cid", "csec").expect("write");

    let app_json_path = config_dir.join("google-chat-cli").join("app.json");
    let content = std::fs::read_to_string(&app_json_path).expect("read");
    let parsed: serde_json::Value = serde_json::from_str(&content).expect("valid JSON");
    let obj = parsed.as_object().expect("should be object");

    assert_eq!(
        obj.len(),
        2,
        "app.json must contain exactly client_id and client_secret when no service_account exists yet"
    );
    assert!(obj.contains_key("client_id"));
    assert!(obj.contains_key("client_secret"));
}

#[test]
fn write_app_config_preserves_existing_service_account_block() {
    // A user may hand-add a `service_account` block for the domain-wide-delegation
    // flow after running `init` once. Re-running `init` (e.g. to rotate the OAuth
    // client secret) must not silently delete that setup.
    let (_dir, config_dir) = temp_config_dir();
    write_app_config(&config_dir, "old-id", "old-secret").expect("first write");

    let app_json_path = config_dir.join("google-chat-cli").join("app.json");
    let hand_edited = serde_json::json!({
        "client_id": "old-id",
        "client_secret": "old-secret",
        "service_account": {
            "client_email": "bot@my-project.iam.gserviceaccount.com",
            "private_key": "-----BEGIN PRIVATE KEY-----\n...\n-----END PRIVATE KEY-----\n",
            "impersonate_user": "service-user@example.com"
        }
    });
    std::fs::write(&app_json_path, serde_json::to_string_pretty(&hand_edited).unwrap())
        .expect("hand-edit app.json");

    write_app_config(&config_dir, "new-id", "new-secret").expect("second write");

    let content = std::fs::read_to_string(&app_json_path).expect("read");
    let parsed: serde_json::Value = serde_json::from_str(&content).expect("valid JSON");

    assert_eq!(parsed["client_id"], "new-id");
    assert_eq!(parsed["client_secret"], "new-secret");
    assert_eq!(
        parsed["service_account"]["client_email"],
        "bot@my-project.iam.gserviceaccount.com"
    );
    assert_eq!(
        parsed["service_account"]["impersonate_user"],
        "service-user@example.com"
    );
}
