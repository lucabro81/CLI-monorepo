#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::path::PathBuf;
use tempfile::TempDir;

use super::{read_hidden_with, resolve_credentials, write_app_config};

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

// ── interactive init on a terminal (issue #196) ──────────────────────────

fn no_prompt(_: &str) -> Result<String, crate::error::CliError> {
    panic!("must not prompt")
}

#[test]
fn both_flags_are_used_as_given_without_prompting() {
    let creds = resolve_credentials(Some("key".into()), Some("org".into()), true, no_prompt, no_prompt).unwrap();

    assert_eq!(creds, Some(("key".to_string(), "org".to_string())));
}

#[test]
fn without_a_terminal_missing_flags_keep_the_skeleton_path() {
    // Unchanged behaviour off a terminal: no prompt, the skeleton file instead.
    for (key, org) in [(None, None), (Some("key".to_string()), None), (None, Some("org".to_string()))] {
        assert_eq!(resolve_credentials(key, org, false, no_prompt, no_prompt).unwrap(), None);
    }
}

#[test]
fn on_a_terminal_the_org_id_is_asked_in_clear_and_the_key_hidden() {
    let creds = resolve_credentials(
        None,
        None,
        true,
        |label| {
            assert_eq!(label, "Organization ID");
            Ok("org-1".to_string())
        },
        |label| {
            assert_eq!(label, "Organization API key");
            Ok("key-1".to_string())
        },
    )
    .unwrap();

    assert_eq!(creds, Some(("key-1".to_string(), "org-1".to_string())));
}

#[test]
fn on_a_terminal_only_the_missing_value_is_asked() {
    let creds = resolve_credentials(None, Some("org-1".into()), true, no_prompt, |_| Ok("key-1".to_string())).unwrap();
    assert_eq!(creds, Some(("key-1".to_string(), "org-1".to_string())));

    let creds = resolve_credentials(Some("key-1".into()), None, true, |_| Ok("org-1".to_string()), no_prompt).unwrap();
    assert_eq!(creds, Some(("key-1".to_string(), "org-1".to_string())));
}

#[test]
fn a_failed_prompt_is_reported() {
    let err = resolve_credentials(
        None,
        Some("org-1".into()),
        true,
        no_prompt,
        |_| Err(crate::error::CliError::IoError { reason: "could not read Organization API key: no tty".to_string() }),
    )
    .unwrap_err();

    assert_eq!(err.to_string(), "I/O error: could not read Organization API key: no tty");
}

#[test]
fn an_empty_answer_writes_nothing() {
    // Regression guard (#196 review): Enter on an empty prompt would have
    // overwritten a valid app.json with an empty key.
    let err = resolve_credentials(None, Some("org-1".into()), true, no_prompt, |_| Ok(String::new())).unwrap_err();
    assert_eq!(
        err.to_string(),
        "Organization API key is empty: nothing was written. Run atlassian-admin init again and type it, \
        or pass it with --api-key"
    );

    let err = resolve_credentials(Some("key-1".into()), None, true, |_| Ok(String::new()), no_prompt).unwrap_err();
    assert_eq!(
        err.to_string(),
        "Organization ID is empty: nothing was written. Run atlassian-admin init again and type it, \
        or pass it with --org-id"
    );
}

#[test]
fn the_hidden_reader_shows_the_label_and_trims() {
    let value = read_hidden_with("Organization API key", |prompt| {
        assert_eq!(prompt, "Organization API key: ");
        Ok("  key-1 \n".to_string())
    })
    .unwrap();

    assert_eq!(value, "key-1");
}

#[test]
fn a_failed_hidden_read_says_how_to_retry() {
    let err = read_hidden_with("Organization API key", |_| Err(std::io::Error::other("no tty"))).unwrap_err();

    assert_eq!(
        err.to_string(),
        "I/O error: could not read Organization API key: no tty. Run atlassian-admin init from a terminal, \
        or pass --api-key and --org-id"
    );
}

