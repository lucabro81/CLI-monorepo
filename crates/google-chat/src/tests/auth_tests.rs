#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::path::{Path, PathBuf};

use super::{
    app_config_path, authorization_url, code_challenge, credentials_path, generate_code_verifier,
    generate_state, jwt_claims, parse_callback_request_line, refresh, CallbackError,
    CallbackParams, Credentials, LoginError, OAuthConfig, OAuthConfigError, ServiceAccountConfig,
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

    assert_eq!(
        code_challenge(verifier),
        "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM"
    );
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
fn state_values_are_random() {
    assert_ne!(generate_state(), generate_state());
}

#[test]
fn builds_authorization_url_with_required_params() {
    let config = OAuthConfig {
        client_id: "my-client-id".to_string(),
        client_secret: "shh".to_string(),
        redirect_uri: "http://localhost:8080/callback".to_string(),
        service_account: None,
    };

    let url = authorization_url(&config, "challenge123", "state456").expect("should build URL");

    assert!(url.starts_with("https://accounts.google.com/o/oauth2/v2/auth?"));
    assert!(url.contains("client_id=my-client-id"));
    assert!(url.contains("code_challenge=challenge123"));
    assert!(url.contains("code_challenge_method=S256"));
    assert!(url.contains("state=state456"));
    assert!(url.contains("response_type=code"));
    assert!(url.contains("access_type=offline"));
    assert!(url.contains("redirect_uri=http%3A%2F%2Flocalhost%3A8080%2Fcallback"));
    assert!(url.contains(
        "scope=https%3A%2F%2Fwww.googleapis.com%2Fauth%2Fchat.spaces.readonly+https%3A%2F%2Fwww.googleapis.com%2Fauth%2Fchat.spaces.create+https%3A%2F%2Fwww.googleapis.com%2Fauth%2Fchat.messages.readonly+https%3A%2F%2Fwww.googleapis.com%2Fauth%2Fchat.messages.create"
    ));
    assert!(url.contains("https%3A%2F%2Fwww.googleapis.com%2Fauth%2Fdirectory.readonly"));
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
fn callback_without_query_string_is_malformed() {
    // No '?' at all — must be MalformedRequestLine, not MissingParam.
    let line = "GET /callback HTTP/1.1";

    assert_eq!(
        parse_callback_request_line(line),
        Err(CallbackError::MalformedRequestLine)
    );
}

#[test]
fn callback_with_extra_query_params_extracts_code_and_state() {
    // Google may append extra params (e.g. scope) — must be ignored.
    let line = "GET /callback?code=abc123&state=xyz789&scope=chat.messages HTTP/1.1";

    let params = parse_callback_request_line(line).expect("should parse");

    assert_eq!(params.code, "abc123");
    assert_eq!(params.state, "xyz789");
}

#[test]
fn callback_with_url_encoded_code_is_decoded() {
    let line = "GET /callback?code=abc%2B123&state=xyz HTTP/1.1";

    let params = parse_callback_request_line(line).expect("should parse");

    assert_eq!(params.code, "abc+123");
}

#[test]
fn credentials_round_trip_through_json_with_refresh_token() {
    let creds = Credentials {
        access_token: "access".to_string(),
        refresh_token: Some("refresh".to_string()),
        expires_at: 1_700_000_000,
    };

    let json = serde_json::to_string(&creds).unwrap();
    let parsed: Credentials = serde_json::from_str(&json).unwrap();

    assert_eq!(parsed, creds);
}

#[test]
fn credentials_round_trip_through_json_without_refresh_token() {
    // Service-account (domain-wide delegation) credentials have no refresh token —
    // a fresh JWT assertion is signed and exchanged on every renewal instead.
    let creds = Credentials {
        access_token: "access".to_string(),
        refresh_token: None,
        expires_at: 1_700_000_000,
    };

    let json = serde_json::to_string(&creds).unwrap();
    let parsed: Credentials = serde_json::from_str(&json).unwrap();

    assert_eq!(parsed, creds);
}

#[test]
fn credentials_without_refresh_token_field_deserializes_to_none() {
    let json = r#"{"access_token": "at", "expires_at": 1000}"#;

    let creds: Credentials = serde_json::from_str(json).unwrap();

    assert_eq!(creds.refresh_token, None);
}

#[test]
fn refresh_without_refresh_token_returns_internal_error() {
    // Service-account credentials (refresh_token: None) cannot be renewed via the
    // refresh_token grant — refresh() must reject this before making any network
    // call, so renew() can fall back to login_service_account instead.
    let config = OAuthConfig {
        client_id: "id".to_string(),
        client_secret: "secret".to_string(),
        redirect_uri: OAuthConfig::REDIRECT_URI.to_string(),
        service_account: None,
    };
    let creds = Credentials {
        access_token: "access".to_string(),
        refresh_token: None,
        expires_at: 0,
    };

    let result = refresh(&config, &creds);

    assert!(matches!(result, Err(LoginError::Internal(_))));
}

#[test]
fn credentials_json_field_names_are_stable() {
    // Regression guard: if serde field names change, existing credentials.json files break.
    let creds = Credentials {
        access_token: "at".to_string(),
        refresh_token: Some("rt".to_string()),
        expires_at: 1_000,
    };

    let json = serde_json::to_string(&creds).unwrap();

    assert!(json.contains("\"access_token\""));
    assert!(json.contains("\"refresh_token\""));
    assert!(json.contains("\"expires_at\""));
}

#[test]
fn credentials_path_is_under_google_chat_cli_dir() {
    let path = credentials_path(Path::new("/home/user/.config"));

    assert_eq!(
        path,
        PathBuf::from("/home/user/.config/google-chat-cli/credentials.json")
    );
}

#[test]
fn app_config_path_is_under_google_chat_cli_dir() {
    let path = app_config_path(Path::new("/home/user/.config"));

    assert_eq!(
        path,
        PathBuf::from("/home/user/.config/google-chat-cli/app.json")
    );
}

#[test]
fn parses_oauth_config_from_app_json_without_service_account() {
    let json = r#"{"client_id": "abc", "client_secret": "shh"}"#;

    let config = OAuthConfig::from_json(json).expect("should parse");

    assert_eq!(
        config,
        OAuthConfig {
            client_id: "abc".to_string(),
            client_secret: "shh".to_string(),
            redirect_uri: OAuthConfig::REDIRECT_URI.to_string(),
            service_account: None,
        }
    );
}

#[test]
fn parses_oauth_config_from_app_json_with_service_account() {
    let json = r#"{
        "client_id": "abc",
        "client_secret": "shh",
        "service_account": {
            "client_email": "bot@my-project.iam.gserviceaccount.com",
            "private_key": "-----BEGIN PRIVATE KEY-----\nMIIB...\n-----END PRIVATE KEY-----\n",
            "impersonate_user": "service-user@example.com"
        }
    }"#;

    let config = OAuthConfig::from_json(json).expect("should parse");

    assert_eq!(
        config.service_account,
        Some(ServiceAccountConfig {
            client_email: "bot@my-project.iam.gserviceaccount.com".to_string(),
            private_key: "-----BEGIN PRIVATE KEY-----\nMIIB...\n-----END PRIVATE KEY-----\n"
                .to_string(),
            impersonate_user: "service-user@example.com".to_string(),
        })
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
fn jwt_claims_carry_impersonation_and_scopes() {
    let sa = ServiceAccountConfig {
        client_email: "bot@my-project.iam.gserviceaccount.com".to_string(),
        private_key: "irrelevant-for-this-test".to_string(),
        impersonate_user: "service-user@example.com".to_string(),
    };

    let claims = jwt_claims(&sa, 1_700_000_000);

    assert_eq!(claims.iss, "bot@my-project.iam.gserviceaccount.com");
    assert_eq!(claims.sub, "service-user@example.com");
    assert_eq!(claims.scope, OAuthConfig::SCOPES);
    assert_eq!(claims.aud, "https://oauth2.googleapis.com/token");
    assert_eq!(claims.iat, 1_700_000_000);
    assert_eq!(claims.exp, 1_700_003_600);
}
