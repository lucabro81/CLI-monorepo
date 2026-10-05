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
    // No access_token: still invalid now that "scope"/"scopes" is optional.
    let body = r#"{"expires_in":7200,"unexpected_field":"x"}"#;

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
        refresh_token: Some("refresh123".to_string()),
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
        refresh_token: None,
    };
    save_credentials(&path, &creds).expect("should save");

    let config = OAuthConfig {
        client_id: "ignored".to_string(),
        client_secret: "ignored".to_string(),
    };
    let loaded = load_credentials(&config, &path).expect("should load without renewing");

    assert_eq!(loaded, creds);
}

fn test_config() -> OAuthConfig {
    OAuthConfig {
        client_id: "abc".to_string(),
        client_secret: "def".to_string(),
    }
}

fn token(refresh_token: Option<&str>) -> TokenResponse {
    TokenResponse {
        access_token: "new-access".to_string(),
        expires_in: 3600,
        scope: "repository pullrequest:write".to_string(),
        refresh_token: refresh_token.map(str::to_string),
    }
}

// ── authorization_code (auth login --user) ─────────────────────────────────

#[test]
fn authorization_url_has_exact_bitbucket_params() {
    let url = authorization_url(&test_config(), "state123").expect("should build");

    assert_eq!(
        url,
        "https://bitbucket.org/site/oauth2/authorize?client_id=abc&response_type=code&state=state123"
    );
}

#[test]
fn authorization_url_encodes_query_values() {
    let config = OAuthConfig {
        client_id: "a b&c".to_string(),
        client_secret: "ignored".to_string(),
    };

    let url = authorization_url(&config, "s=1").expect("should build");

    assert_eq!(
        url,
        "https://bitbucket.org/site/oauth2/authorize?client_id=a+b%26c&response_type=code&state=s%3D1"
    );
}

// State and callback parsing are tested in crates/oauth-user-login; here only
// the wrapping, which must add this CLI's retry command.

#[test]
fn denied_consent_ends_with_the_retry_command() {
    let err = LoginError::Callback(oauth_user_login::WaitError::Callback(
        oauth_user_login::CallbackError::Denied {
            error: "access_denied".to_string(),
            description: Some("User denied access".to_string()),
        },
    ));

    assert_eq!(
        err.to_string(),
        "authorization denied: access_denied (User denied access). Approve the consent page to \
        log in, then retry the login: bitbucket auth login --user"
    );
}

#[test]
fn state_mismatch_ends_with_the_retry_command() {
    let err = LoginError::Callback(oauth_user_login::WaitError::StateMismatch);

    assert!(err.to_string().ends_with("Login aborted: retry it: bitbucket auth login --user"), "got {err}");
}

#[test]
fn a_busy_callback_port_names_the_port_and_the_retry_command() {
    let busy = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let address = busy.local_addr().unwrap().to_string();
    let err = LoginError::CallbackListener(oauth_user_login::bind_listener(&address).unwrap_err());

    assert!(err.to_string().starts_with(&format!("cannot listen for the login callback on {address}")), "got {err}");
    assert!(err.to_string().ends_with("and retry: bitbucket auth login --user"), "got {err}");
}

#[test]
fn authorization_code_form_has_grant_and_code() {
    assert_eq!(
        authorization_code_form("XYZ"),
        vec![("grant_type", "authorization_code".to_string()), ("code", "XYZ".to_string())]
    );
}

// ── token responses ────────────────────────────────────────────────────────

#[test]
fn deserializes_token_response_with_plural_scopes_and_refresh_token() {
    // Bitbucket's documented authorization_code/refresh_token responses use
    // "scopes" (plural), while the observed client_credentials response uses
    // "scope". Both must be accepted.
    let body = r#"{"access_token":"tok","scopes":"repository account","expires_in":3600,"refresh_token":"ref","token_type":"bearer","state":"authorization_code"}"#;

    let token = parse_token_response(body).expect("should parse");

    assert_eq!(token.scope, "repository account");
    assert_eq!(token.refresh_token.as_deref(), Some("ref"));
}

#[test]
fn deserializes_token_response_without_any_scope_field() {
    let body = r#"{"access_token":"tok","expires_in":3600}"#;

    let token = parse_token_response(body).expect("should parse");

    assert_eq!(token.scope, "");
    assert_eq!(token.refresh_token, None);
}

#[test]
fn credentials_from_token_computes_expiry_and_splits_scopes() {
    let credentials = credentials_from_token(token(Some("new-refresh")), 1_000, None);

    assert_eq!(
        credentials,
        Credentials {
            access_token: "new-access".to_string(),
            expires_at: 4_600,
            scopes: vec!["repository".to_string(), "pullrequest:write".to_string()],
            refresh_token: Some("new-refresh".to_string()),
        }
    );
}

#[test]
fn credentials_from_token_prefers_rotated_refresh_token() {
    let credentials = credentials_from_token(token(Some("rotated")), 0, Some("old".to_string()));

    assert_eq!(credentials.refresh_token.as_deref(), Some("rotated"));
}

#[test]
fn credentials_from_token_keeps_previous_refresh_token_when_absent() {
    // Guards against a silent identity switch: if a refresh response ever
    // omitted refresh_token, dropping the old one would make the next renewal
    // fall back to client_credentials — i.e. quietly start acting as the app
    // instead of the human who ran `auth login --user`.
    let credentials = credentials_from_token(token(None), 0, Some("old".to_string()));

    assert_eq!(credentials.refresh_token.as_deref(), Some("old"));
}

#[test]
fn deserializes_legacy_credentials_without_refresh_token() {
    // credentials.json files written before `auth login --user` existed have
    // no refresh_token field and must keep loading as app credentials.
    let json = r#"{"access_token":"tok","expires_at":123,"scopes":["repository"]}"#;

    let credentials: Credentials = serde_json::from_str(json).expect("should parse legacy file");

    assert_eq!(credentials.refresh_token, None);
}

// ── renewal ────────────────────────────────────────────────────────────────

#[test]
fn renewal_form_uses_refresh_token_for_user_credentials() {
    let credentials = Credentials {
        access_token: "tok".to_string(),
        expires_at: 0,
        scopes: vec![],
        refresh_token: Some("ref".to_string()),
    };

    assert_eq!(
        renewal_form(&credentials),
        vec![("grant_type", "refresh_token".to_string()), ("refresh_token", "ref".to_string())]
    );
}

#[test]
fn renewal_form_uses_client_credentials_for_app_credentials() {
    let credentials = Credentials {
        access_token: "tok".to_string(),
        expires_at: 0,
        scopes: vec![],
        refresh_token: None,
    };

    assert_eq!(
        renewal_form(&credentials),
        vec![("grant_type", "client_credentials".to_string())]
    );
}
