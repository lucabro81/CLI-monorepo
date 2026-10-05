#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::path::Path;

use crate::test_support::one_shot_server;

use super::{
    AppConfig, AppConfigError, Credentials, LoginError, ServiceUserKey, app_config_path,
    build_assertion, credentials_path, is_expiring, jwt_claims, load_credentials,
    login_service_user, save_credentials,
};

/// Throwaway RSA key pair generated only for these tests (PKCS#1, the format
/// ZITADEL puts in a downloaded service-user key file). Not used anywhere else.
const TEST_PRIVATE_KEY: &str = include_str!("fixtures/test_rsa_private_key.pem");
const TEST_PUBLIC_KEY: &str = include_str!("fixtures/test_rsa_public_key.pem");

fn test_key() -> ServiceUserKey {
    ServiceUserKey {
        key_id: "100509901696068329".to_string(),
        key: TEST_PRIVATE_KEY.to_string(),
        user_id: "100507859606888466".to_string(),
    }
}

fn config_with(instance_url: &str, service_user: Option<ServiceUserKey>) -> AppConfig {
    AppConfig {
        instance_url: instance_url.to_string(),
        service_user,
        client_id: None,
    }
}

// ── app.json ──────────────────────────────────────────────────────────────

#[test]
fn parses_app_json_with_only_instance_url() {
    let config = AppConfig::from_json(r#"{"instance_url": "https://acme.zitadel.cloud"}"#).unwrap();

    assert_eq!(config, config_with("https://acme.zitadel.cloud", None));
}

#[test]
fn parses_app_json_with_service_user_in_console_key_file_format() {
    // The service_user block is the ZITADEL key file copied verbatim, including
    // its "type" field, so `init` never has to translate field names.
    let json = r#"{
        "instance_url": "https://acme.zitadel.cloud",
        "service_user": {
            "type": "serviceaccount",
            "keyId": "100509901696068329",
            "key": "-----BEGIN RSA PRIVATE KEY-----\nMIIE...\n-----END RSA PRIVATE KEY-----\n",
            "userId": "100507859606888466"
        },
        "client_id": "123456789@zitadel-cli"
    }"#;

    let config = AppConfig::from_json(json).unwrap();

    assert_eq!(
        config,
        AppConfig {
            instance_url: "https://acme.zitadel.cloud".to_string(),
            service_user: Some(ServiceUserKey {
                key_id: "100509901696068329".to_string(),
                key: "-----BEGIN RSA PRIVATE KEY-----\nMIIE...\n-----END RSA PRIVATE KEY-----\n"
                    .to_string(),
                user_id: "100507859606888466".to_string(),
            }),
            client_id: Some("123456789@zitadel-cli".to_string()),
        }
    );
}

#[test]
fn strips_trailing_slashes_from_instance_url() {
    // The instance URL becomes the JWT `aud` claim; ZITADEL compares it exactly,
    // so "https://x/" (as copied from a browser bar) must equal "https://x".
    let config = AppConfig::from_json(r#"{"instance_url": "https://acme.zitadel.cloud//"}"#).unwrap();

    assert_eq!(config.instance_url, "https://acme.zitadel.cloud");
}

#[test]
fn accepts_http_instance_url_for_local_self_hosted() {
    let config = AppConfig::from_json(r#"{"instance_url": "http://localhost:8081"}"#).unwrap();

    assert_eq!(config.instance_url, "http://localhost:8081");
}

#[test]
fn rejects_instance_url_without_scheme() {
    let err = AppConfig::from_json(r#"{"instance_url": "acme.zitadel.cloud"}"#).unwrap_err();

    assert_eq!(
        err,
        AppConfigError::InvalidInstanceUrl("acme.zitadel.cloud".to_string())
    );
}

#[test]
fn rejects_empty_instance_url() {
    let err = AppConfig::from_json(r#"{"instance_url": ""}"#).unwrap_err();

    assert_eq!(err, AppConfigError::InvalidInstanceUrl(String::new()));
}

#[test]
fn rejects_app_json_missing_instance_url() {
    let err = AppConfig::from_json(r"{}").unwrap_err();

    assert!(
        matches!(&err, AppConfigError::InvalidJson(msg) if msg.contains("instance_url")),
        "got {err:?}"
    );
}

#[test]
fn rejects_service_user_missing_key_id() {
    let json = r#"{"instance_url": "https://acme.zitadel.cloud",
                   "service_user": {"key": "k", "userId": "u"}}"#;

    let err = AppConfig::from_json(json).unwrap_err();

    assert!(
        matches!(&err, AppConfigError::InvalidJson(msg) if msg.contains("keyId")),
        "got {err:?}"
    );
}

#[test]
fn rejects_malformed_app_json() {
    assert!(matches!(
        AppConfig::from_json("not json"),
        Err(AppConfigError::InvalidJson(_))
    ));
}

#[test]
fn load_reports_not_found_with_path() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("missing.json");

    assert_eq!(AppConfig::load(&path), Err(AppConfigError::NotFound(path)));
}

#[test]
fn config_paths_are_under_zitadel_cli_dir() {
    let base = Path::new("/home/u/.config");

    assert_eq!(app_config_path(base), base.join("zitadel-cli").join("app.json"));
    assert_eq!(
        credentials_path(base),
        base.join("zitadel-cli").join("credentials.json")
    );
}

// ── JWT assertion ─────────────────────────────────────────────────────────

#[test]
fn jwt_claims_follow_zitadel_private_key_jwt_spec() {
    let claims = jwt_claims(&test_key(), "https://acme.zitadel.cloud", 1_700_000_000);

    assert_eq!(claims.iss, "100507859606888466");
    assert_eq!(claims.sub, "100507859606888466");
    assert_eq!(claims.aud, "https://acme.zitadel.cloud");
    assert_eq!(claims.iat, 1_700_000_000);
    assert_eq!(claims.exp, 1_700_003_600);
}

#[test]
fn assertion_is_rs256_with_kid_and_verifies_against_public_key() {
    use jsonwebtoken::{Algorithm, DecodingKey, Validation, decode, decode_header};

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    let token = build_assertion(&test_key(), "https://acme.zitadel.cloud", now).unwrap();

    let header = decode_header(&token).unwrap();
    assert_eq!(header.alg, Algorithm::RS256);
    assert_eq!(header.kid.as_deref(), Some("100509901696068329"));

    let mut validation = Validation::new(Algorithm::RS256);
    validation.set_audience(&["https://acme.zitadel.cloud"]);
    validation.set_issuer(&["100507859606888466"]);
    let decoded = decode::<serde_json::Value>(
        &token,
        &DecodingKey::from_rsa_pem(TEST_PUBLIC_KEY.as_bytes()).unwrap(),
        &validation,
    )
    .unwrap();
    assert_eq!(decoded.claims["sub"], "100507859606888466");
}

#[test]
fn assertion_with_invalid_private_key_is_an_actionable_error() {
    let mut key = test_key();
    key.key = "not a pem".to_string();

    let err = build_assertion(&key, "https://acme.zitadel.cloud", 0).unwrap_err();

    assert!(
        matches!(&err, LoginError::InvalidPrivateKey(_)),
        "got {err:?}"
    );
    assert!(err.to_string().contains("zitadel init --key-file"));
}

// ── token exchange (against a one-shot local mock server) ─────────────────

#[test]
fn login_service_user_posts_jwt_bearer_form_to_instance_token_endpoint() {
    let (url, server) = one_shot_server(
        "200 OK",
        r#"{"access_token":"at-123","token_type":"Bearer","expires_in":43199,"id_token":"x"}"#,
    );

    let creds = login_service_user(&config_with(&url, Some(test_key()))).unwrap();

    let request = server.join().unwrap();
    assert!(request.starts_with("POST /oauth/v2/token "), "got {request}");
    let form: std::collections::HashMap<String, String> =
        serde_urlencoded::from_str(request.lines().last().unwrap()).unwrap();
    assert_eq!(form["grant_type"], "urn:ietf:params:oauth:grant-type:jwt-bearer");
    assert_eq!(form["scope"], "openid urn:zitadel:iam:org:project:id:zitadel:aud");
    assert_eq!(
        jsonwebtoken::decode_header(&form["assertion"]).unwrap().kid.as_deref(),
        Some("100509901696068329")
    );

    assert_eq!(creds.access_token, "at-123");
    assert_eq!(creds.refresh_token, None);
}

#[test]
fn login_service_user_surfaces_zitadel_error_body() {
    let (url, server) = one_shot_server(
        "400 Bad Request",
        r#"{"error":"invalid_grant","error_description":"assertion invalid"}"#,
    );

    let err = login_service_user(&config_with(&url, Some(test_key()))).unwrap_err();
    server.join().unwrap();

    match err {
        LoginError::TokenExchange(msg) => {
            assert!(msg.contains("400"), "got {msg}");
            assert!(msg.contains("assertion invalid"), "got {msg}");
        }
        other => panic!("expected TokenExchange, got {other:?}"),
    }
}

#[test]
fn login_service_user_without_service_user_block_is_an_actionable_error() {
    let err = login_service_user(&config_with("https://acme.zitadel.cloud", None)).unwrap_err();

    assert!(matches!(err, LoginError::ServiceUserNotConfigured));
    assert!(err.to_string().contains("zitadel init --key-file"));
}

// ── credentials persistence and renewal ───────────────────────────────────

#[test]
fn credentials_round_trip_through_disk() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("zitadel-cli").join("credentials.json");
    let creds = Credentials {
        access_token: "at".to_string(),
        refresh_token: Some("rt".to_string()),
        expires_at: 4_000_000_000,
    };

    save_credentials(&path, &creds).unwrap();

    let raw: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    assert_eq!(
        raw,
        serde_json::json!({"access_token": "at", "refresh_token": "rt", "expires_at": 4_000_000_000_u64})
    );
    // Far-future expiry: returned as-is, no network call.
    let config = config_with("https://unused.invalid", None);
    assert_eq!(load_credentials(&config, &path).unwrap(), creds);
}

#[test]
fn credentials_without_refresh_token_field_deserialize_to_none() {
    let creds: Credentials =
        serde_json::from_str(r#"{"access_token":"at","expires_at":1}"#).unwrap();

    assert_eq!(creds.refresh_token, None);
}

#[test]
fn load_credentials_renews_expired_service_user_token_and_persists_it() {
    let (url, server) = one_shot_server(
        "200 OK",
        r#"{"access_token":"fresh","token_type":"Bearer","expires_in":43199}"#,
    );
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("credentials.json");
    save_credentials(
        &path,
        &Credentials {
            access_token: "stale".to_string(),
            refresh_token: None,
            expires_at: 0,
        },
    )
    .unwrap();

    let creds = load_credentials(&config_with(&url, Some(test_key())), &path).unwrap();
    server.join().unwrap();

    assert_eq!(creds.access_token, "fresh");
    let on_disk: Credentials = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    assert_eq!(on_disk, creds);
}

#[test]
fn token_is_expiring_within_60_second_leeway() {
    assert!(is_expiring(1_000, 1_000));
    assert!(is_expiring(1_060, 1_000));
    assert!(!is_expiring(1_061, 1_000));
    assert!(is_expiring(0, 1_000));
}
