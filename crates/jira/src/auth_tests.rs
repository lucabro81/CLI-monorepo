#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::path::{Path, PathBuf};

use super::{
    app_config_path, authorization_url, code_challenge, credentials_path, generate_code_verifier,
    generate_state, parse_callback_request_line, CallbackError, CallbackParams, Credentials,
    OAuthConfig, OAuthConfigError,
};

#[test]
fn code_verifier_is_url_safe_and_long_enough() {
    let verifier = generate_code_verifier();

    assert!(verifier.len() >= 43 && verifier.len() <= 128);
    assert!(verifier
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_'));
}

#[test]
fn code_verifiers_are_random() {
    assert_ne!(generate_code_verifier(), generate_code_verifier());
}

#[test]
fn code_challenge_matches_known_rfc7636_example() {
    // From RFC 7636 appendix B.
    let verifier = "dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk";

    assert_eq!(code_challenge(verifier), "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM");
}

#[test]
fn state_values_are_random() {
    assert_ne!(generate_state(), generate_state());
}

#[test]
fn builds_authorization_url_with_required_params() {
    let config = OAuthConfig {
        client_id: "my-client-id".to_string(),
        client_secret: "shh".to_string(),
        redirect_uri: "http://localhost:8080/callback".to_string(),
    };

    let url = authorization_url(&config, "challenge123", "state456").expect("should build URL");

    assert!(url.starts_with("https://auth.atlassian.com/authorize?"));
    assert!(url.contains("client_id=my-client-id"));
    assert!(url.contains("code_challenge=challenge123"));
    assert!(url.contains("code_challenge_method=S256"));
    assert!(url.contains("state=state456"));
    assert!(url.contains("response_type=code"));
    assert!(url.contains("audience=api.atlassian.com"));
    assert!(url.contains("redirect_uri=http%3A%2F%2Flocalhost%3A8080%2Fcallback"));
    assert!(url.contains(
        "scope=read%3Ajira-work+read%3Ajira-user+write%3Ajira-work+offline_access"
    ));
}

#[test]
fn parses_valid_callback_request_line() {
    let line = "GET /callback?code=abc123&state=xyz789 HTTP/1.1";

    let params = parse_callback_request_line(line).expect("should parse");

    assert_eq!(
        params,
        CallbackParams {
            code: "abc123".to_string(),
            state: "xyz789".to_string(),
        }
    );
}

#[test]
fn rejects_callback_missing_code() {
    let line = "GET /callback?state=xyz789 HTTP/1.1";

    assert_eq!(
        parse_callback_request_line(line),
        Err(CallbackError::MissingParam("code"))
    );
}

#[test]
fn rejects_callback_missing_state() {
    let line = "GET /callback?code=abc123 HTTP/1.1";

    assert_eq!(
        parse_callback_request_line(line),
        Err(CallbackError::MissingParam("state"))
    );
}

#[test]
fn rejects_malformed_request_line() {
    assert_eq!(
        parse_callback_request_line("not a request line"),
        Err(CallbackError::MalformedRequestLine)
    );
}

#[test]
fn credentials_round_trip_through_json() {
    let creds = Credentials {
        access_token: "access".to_string(),
        refresh_token: "refresh".to_string(),
        expires_at: 1_700_000_000,
        cloud_id: "cloud-123".to_string(),
    };

    let json = serde_json::to_string(&creds).unwrap();
    let parsed: Credentials = serde_json::from_str(&json).unwrap();

    assert_eq!(parsed, creds);
}

#[test]
fn credentials_path_is_under_jira_cli_dir() {
    let path = credentials_path(Path::new("/home/user/.config"));

    assert_eq!(
        path,
        PathBuf::from("/home/user/.config/jira-cli/credentials.json")
    );
}

#[test]
fn app_config_path_is_under_jira_cli_dir() {
    let path = app_config_path(Path::new("/home/user/.config"));

    assert_eq!(
        path,
        PathBuf::from("/home/user/.config/jira-cli/app.json")
    );
}

#[test]
fn parses_oauth_config_from_app_json() {
    let json = r#"{"client_id": "abc", "client_secret": "shh"}"#;

    let config = OAuthConfig::from_json(json).expect("should parse");

    assert_eq!(
        config,
        OAuthConfig {
            client_id: "abc".to_string(),
            client_secret: "shh".to_string(),
            redirect_uri: OAuthConfig::REDIRECT_URI.to_string(),
        }
    );
}

#[test]
fn rejects_malformed_app_json() {
    let result = OAuthConfig::from_json("not json");

    assert!(matches!(result, Err(OAuthConfigError::InvalidJson(_))));
}

#[test]
fn rejects_app_json_missing_client_id() {
    let result = OAuthConfig::from_json(r#"{"client_secret": "shh"}"#);

    assert!(matches!(result, Err(OAuthConfigError::InvalidJson(_))));
}

#[test]
fn rejects_app_json_missing_client_secret() {
    let result = OAuthConfig::from_json(r#"{"client_id": "abc"}"#);

    assert!(matches!(result, Err(OAuthConfigError::InvalidJson(_))));
}

#[test]
fn accepts_app_json_with_extra_fields() {
    // serde ignores unknown fields — extra keys in app.json must not break loading.
    let json = r#"{"client_id": "abc", "client_secret": "shh", "extra": "ignored"}"#;

    let config = OAuthConfig::from_json(json).expect("should parse");

    assert_eq!(config.client_id, "abc");
    assert_eq!(config.client_secret, "shh");
}

#[test]
fn code_challenge_is_url_safe_and_unpadded() {
    // RFC 7636 §4.2: base64url encoding without padding; no +, /, or = characters.
    let challenge = code_challenge("any-verifier");

    assert!(!challenge.contains('='), "must not contain padding '='");
    assert!(!challenge.contains('+'), "must not contain '+'");
    assert!(!challenge.contains('/'), "must not contain '/'");
}

#[test]
fn callback_with_extra_query_params_extracts_code_and_state() {
    // Atlassian (or future Jira versions) may append extra params — must be ignored.
    let line = "GET /callback?code=abc123&state=xyz789&scope=read%3Ajira-work HTTP/1.1";

    let params = parse_callback_request_line(line).expect("should parse");

    assert_eq!(params.code, "abc123");
    assert_eq!(params.state, "xyz789");
}

#[test]
fn callback_with_url_encoded_code_is_decoded() {
    // Authorization codes can contain characters that get percent-encoded in the redirect.
    let line = "GET /callback?code=abc%2B123&state=xyz HTTP/1.1";

    let params = parse_callback_request_line(line).expect("should parse");

    assert_eq!(params.code, "abc+123");
}

#[test]
fn callback_without_query_string_is_malformed() {
    // No '?' at all — must be MalformedRequestLine, not MissingParam.
    let line = "GET /callback HTTP/1.1";

    assert_eq!(
        parse_callback_request_line(line),
        Err(CallbackError::MalformedRequestLine)
    );
}

#[test]
fn callback_with_empty_query_string_missing_code() {
    let line = "GET /callback? HTTP/1.1";

    assert_eq!(
        parse_callback_request_line(line),
        Err(CallbackError::MissingParam("code"))
    );
}

#[test]
fn credentials_json_field_names_are_stable() {
    // Regression guard: if serde field names change, existing credentials.json files break.
    let creds = Credentials {
        access_token: "at".to_string(),
        refresh_token: "rt".to_string(),
        expires_at: 1_000,
        cloud_id: "cid".to_string(),
    };

    let json = serde_json::to_string(&creds).unwrap();

    assert!(json.contains("\"access_token\""));
    assert!(json.contains("\"refresh_token\""));
    assert!(json.contains("\"expires_at\""));
    assert!(json.contains("\"cloud_id\""));
}
