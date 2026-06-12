#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::path::Path;

use super::*;

#[test]
fn parses_valid_app_credentials() {
    let json = r#"{"client_id": "abc", "client_secret": "def"}"#;

    let config = OAuthConfig::from_json(json).expect("should parse");

    assert_eq!(config.client_id, "abc");
    assert_eq!(config.client_secret, "def");
}

#[test]
fn rejects_invalid_app_credentials_json() {
    let result = OAuthConfig::from_json("not json");

    assert!(matches!(result, Err(OAuthConfigError::InvalidJson(_))));
}

#[test]
fn load_returns_not_found_for_missing_file() {
    let result = OAuthConfig::load(Path::new("/nonexistent/app.json"));

    assert!(matches!(result, Err(OAuthConfigError::NotFound(_))));
}

#[test]
fn app_config_path_is_under_bitbucket_cli_dir() {
    let path = app_config_path(Path::new("/home/user/.config"));

    assert_eq!(path, Path::new("/home/user/.config/bitbucket-cli/app.json"));
}

#[test]
fn credentials_path_is_under_bitbucket_cli_dir() {
    let path = credentials_path(Path::new("/home/user/.config"));

    assert_eq!(
        path,
        Path::new("/home/user/.config/bitbucket-cli/credentials.json")
    );
}

#[test]
fn credentials_round_trip_through_json() {
    let creds = Credentials {
        access_token: "token123".to_string(),
        expires_at: 1_700_000_000,
        scopes: vec!["repository:read".to_string(), "pullrequest:write".to_string()],
    };

    let json = serde_json::to_string(&creds).expect("should serialize");
    let parsed: Credentials = serde_json::from_str(&json).expect("should deserialize");

    assert_eq!(parsed, creds);
}

#[test]
fn save_and_load_credentials_roundtrip_without_expiry() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("credentials.json");

    let creds = Credentials {
        access_token: "token123".to_string(),
        // Far in the future, so load_credentials doesn't try to renew over the network.
        expires_at: u64::MAX,
        scopes: vec!["repository:read".to_string()],
    };
    save_credentials(&path, &creds).expect("should save");

    let config = OAuthConfig {
        client_id: "ignored".to_string(),
        client_secret: "ignored".to_string(),
    };
    let loaded = load_credentials(&config, &path).expect("should load without renewing");

    assert_eq!(loaded, creds);
}
