#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::path::Path;

use super::*;

#[test]
fn parses_valid_app_credentials() {
    let json = r#"{"api_key": "abc", "org_id": "def"}"#;

    let config = AdminConfig::from_json(json).expect("should parse");

    assert_eq!(config.api_key, "abc");
    assert_eq!(config.org_id, "def");
}

#[test]
fn rejects_invalid_app_credentials_json() {
    let result = AdminConfig::from_json("not json");

    assert!(matches!(result, Err(AdminConfigError::InvalidJson(_))));
}

#[test]
fn load_returns_not_found_for_missing_file() {
    let result = AdminConfig::load(Path::new("/nonexistent/app.json"));

    assert!(matches!(result, Err(AdminConfigError::NotFound(_))));
}

#[test]
fn app_config_path_is_under_atlassian_admin_cli_dir() {
    let path = app_config_path(Path::new("/home/user/.config"));

    assert_eq!(path, Path::new("/home/user/.config/atlassian-admin-cli/app.json"));
}
