#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::path::PathBuf;

use crate::auth::Identity;
use serde_json::json;
use tempfile::TempDir;

use super::write_app_config;
use crate::error::CliError;

fn temp_config_dir() -> (TempDir, PathBuf) {
    let dir = TempDir::new().expect("tempdir");
    let path = dir.path().to_path_buf();
    (dir, path)
}

fn read_app_json(config_dir: &std::path::Path) -> serde_json::Value {
    let content = std::fs::read_to_string(config_dir.join("bitbucket-cli").join("app.json")).expect("read");
    serde_json::from_str(&content).expect("valid JSON")
}

#[test]
fn write_app_config_writes_only_the_section_of_the_identity() {
    let (_dir, config_dir) = temp_config_dir();

    write_app_config(&config_dir, Identity::Service, "svc-id", "svc-secret").expect("should write");

    assert_eq!(
        read_app_json(&config_dir),
        json!({"service": {"client_id": "svc-id", "client_secret": "svc-secret"}})
    );
}

#[test]
fn write_app_config_keeps_the_other_identity_section() {
    // Configuring the 3LO app must not erase the Service Account, and vice versa.
    let (_dir, config_dir) = temp_config_dir();

    write_app_config(&config_dir, Identity::Service, "svc-id", "svc-secret").expect("first write");
    write_app_config(&config_dir, Identity::User, "usr-id", "usr-secret").expect("second write");

    assert_eq!(
        read_app_json(&config_dir),
        json!({
            "service": {"client_id": "svc-id", "client_secret": "svc-secret"},
            "user": {"client_id": "usr-id", "client_secret": "usr-secret"}
        })
    );
}

#[test]
fn write_app_config_replaces_the_same_section() {
    let (_dir, config_dir) = temp_config_dir();

    write_app_config(&config_dir, Identity::User, "old-id", "old-secret").expect("first write");
    write_app_config(&config_dir, Identity::User, "new-id", "new-secret").expect("second write");

    assert_eq!(
        read_app_json(&config_dir),
        json!({"user": {"client_id": "new-id", "client_secret": "new-secret"}})
    );
}

#[test]
fn write_app_config_replaces_a_legacy_flat_file() {
    // The legacy-format error tells the caller to re-run init, so init must
    // accept that file and replace it rather than failing on it again.
    let (_dir, config_dir) = temp_config_dir();
    std::fs::create_dir_all(config_dir.join("bitbucket-cli")).unwrap();
    std::fs::write(
        config_dir.join("bitbucket-cli").join("app.json"),
        r#"{"client_id": "old", "client_secret": "old"}"#,
    )
    .unwrap();

    write_app_config(&config_dir, Identity::Service, "svc-id", "svc-secret").expect("should write");

    assert_eq!(
        read_app_json(&config_dir),
        json!({"service": {"client_id": "svc-id", "client_secret": "svc-secret"}})
    );
}

#[test]
fn write_app_config_refuses_to_overwrite_an_unreadable_file() {
    let (_dir, config_dir) = temp_config_dir();
    let path = config_dir.join("bitbucket-cli").join("app.json");
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(&path, "not json").unwrap();

    let result = write_app_config(&config_dir, Identity::Service, "svc-id", "svc-secret");

    assert!(matches!(result, Err(CliError::AppConfigInvalid { .. })), "{result:?}");
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "not json");
}

#[test]
fn write_app_config_creates_parent_directories() {
    // config_dir may not exist yet on a fresh machine
    let (_dir, config_dir) = temp_config_dir();
    let nested = config_dir.join("does").join("not").join("exist");

    write_app_config(&nested, Identity::User, "id", "secret").expect("should create dirs and write");

    assert!(nested.join("bitbucket-cli").join("app.json").exists());
}
