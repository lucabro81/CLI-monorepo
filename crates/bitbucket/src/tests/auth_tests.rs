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
fn deserializes_real_bitbucket_token_response_shape() {
    // Regression test: Bitbucket's client_credentials token endpoint (issued via
    // auth.atlassian.com) returns the standard OAuth2 field name "scope"
    // (singular, RFC 6749), not "scopes" (plural). TokenResponse previously
    // required "scopes", so real responses failed to deserialize with
    // "error decoding response body".
    let json = r#"{"access_token":"tok","token_type":"Bearer","expires_in":7200,"scope":"repository:read pullrequest:write"}"#;

    let token: TokenResponse =
        serde_json::from_str(json).expect("should parse real Bitbucket response shape");

    assert_eq!(token.access_token, "tok");
    assert_eq!(token.expires_in, 7200);
    assert_eq!(token.scope, "repository:read pullrequest:write");
}

#[test]
fn parse_token_response_includes_raw_body_on_invalid_json() {
    // Guards against a diagnosability gap: when the token endpoint returns
    // 200 with a body that doesn't match TokenResponse (e.g. an unexpected
    // field name, as happened with "scope" vs "scopes"), the error must
    // include the raw body so the failure is self-diagnosing instead of
    // requiring a manual curl to see what the server actually sent.
    let body = r#"{"access_token":"tok","expires_in":7200,"unexpected_field":"x"}"#;

    let err = parse_token_response(body).expect_err("should fail to parse");

    assert!(
        matches!(&err, LoginError::TokenExchange(msg) if msg.contains(body)),
        "expected error to contain raw body {body:?}, got {err}"
    );
}

#[test]
fn parse_token_response_succeeds_on_valid_json() {
    let body = r#"{"access_token":"tok","expires_in":7200,"scope":"repository:read"}"#;

    let token = parse_token_response(body).expect("should parse");

    assert_eq!(token.access_token, "tok");
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
