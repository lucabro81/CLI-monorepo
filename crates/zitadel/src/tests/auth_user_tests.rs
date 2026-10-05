#![allow(clippy::unwrap_used, clippy::expect_used)]
//! Tests for the human (`auth login --user`) flow: PKCE, authorization URL,
//! loopback callback, code exchange, refresh, and renewal dispatch.

use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::thread;

use super::{
    AppConfig, CallbackError, CallbackParams, Credentials, LoginError, authorization_url,
    code_challenge, exchange_code, generate_code_verifier, generate_state, load_credentials,
    parse_callback_request_line, refresh, save_credentials, wait_for_callback,
};
use crate::test_support::one_shot_server;

fn native_config(instance_url: &str) -> AppConfig {
    AppConfig {
        instance_url: instance_url.to_string(),
        service_user: None,
        client_id: Some("123@zitadel-cli".to_string()),
    }
}

fn form(request: &str) -> HashMap<String, String> {
    serde_urlencoded::from_str(request.lines().last().unwrap()).unwrap()
}

// ── PKCE / state ──────────────────────────────────────────────────────────

#[test]
fn code_challenge_matches_rfc7636_example() {
    assert_eq!(
        code_challenge("dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk"),
        "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM"
    );
}

#[test]
fn code_verifier_is_url_safe_random_and_within_rfc_length() {
    let verifier = generate_code_verifier();

    assert!((43..=128).contains(&verifier.len()), "len {}", verifier.len());
    assert!(verifier.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_'));
    assert_ne!(verifier, generate_code_verifier());
}

#[test]
fn state_values_are_random() {
    assert_ne!(generate_state(), generate_state());
}

// ── authorization URL ─────────────────────────────────────────────────────

#[test]
fn authorization_url_carries_pkce_scopes_and_redirect() {
    let url = authorization_url(&native_config("https://acme.zitadel.cloud"), "chal", "st").unwrap();

    let (base, query) = url.split_once('?').unwrap();
    assert_eq!(base, "https://acme.zitadel.cloud/oauth/v2/authorize");
    let params: HashMap<String, String> = serde_urlencoded::from_str(query).unwrap();
    assert_eq!(
        params,
        HashMap::from([
            ("client_id".to_string(), "123@zitadel-cli".to_string()),
            ("redirect_uri".to_string(), "http://localhost:8080/callback".to_string()),
            ("response_type".to_string(), "code".to_string()),
            (
                "scope".to_string(),
                "openid profile email offline_access urn:zitadel:iam:org:project:id:zitadel:aud"
                    .to_string()
            ),
            ("state".to_string(), "st".to_string()),
            ("code_challenge".to_string(), "chal".to_string()),
            ("code_challenge_method".to_string(), "S256".to_string()),
        ])
    );
}

#[test]
fn authorization_url_without_client_id_points_to_init() {
    let config = AppConfig { client_id: None, ..native_config("https://acme.zitadel.cloud") };

    let err = authorization_url(&config, "c", "s").unwrap_err();

    assert!(matches!(err, LoginError::NativeAppNotConfigured), "got {err:?}");
    assert_eq!(
        err.to_string(),
        "no Native app configured: app.json has no \"client_id\". Create a Native application \
        (PKCE, redirect http://localhost:8080/callback, refresh token enabled) in a ZITADEL project \
        and run: zitadel init --client-id <client-id>"
    );
}

// ── callback parsing ──────────────────────────────────────────────────────

#[test]
fn parses_callback_with_code_and_state() {
    assert_eq!(
        parse_callback_request_line("GET /callback?code=abc%2F1&state=xyz HTTP/1.1"),
        Ok(CallbackParams { code: "abc/1".to_string(), state: "xyz".to_string() })
    );
}

#[test]
fn callback_with_error_reports_the_denial() {
    assert_eq!(
        parse_callback_request_line(
            "GET /callback?error=access_denied&error_description=user+cancelled&state=xyz HTTP/1.1"
        ),
        Err(CallbackError::Denied("access_denied: user cancelled".to_string()))
    );
}

#[test]
fn callback_missing_code_or_state_is_rejected() {
    assert_eq!(
        parse_callback_request_line("GET /callback?state=xyz HTTP/1.1"),
        Err(CallbackError::MissingParam("code"))
    );
    assert_eq!(
        parse_callback_request_line("GET /callback?code=abc HTTP/1.1"),
        Err(CallbackError::MissingParam("state"))
    );
}

#[test]
fn non_callback_requests_are_not_callbacks() {
    assert_eq!(
        parse_callback_request_line("GET /favicon.ico HTTP/1.1"),
        Err(CallbackError::NotCallback)
    );
    assert_eq!(parse_callback_request_line("garbage"), Err(CallbackError::NotCallback));
}

// ── loopback listener ─────────────────────────────────────────────────────

fn send(addr: std::net::SocketAddr, request_line: &str) -> String {
    let mut stream = TcpStream::connect(addr).unwrap();
    stream.write_all(format!("{request_line}\r\nHost: localhost\r\n\r\n").as_bytes()).unwrap();
    let mut response = String::new();
    stream.read_to_string(&mut response).unwrap();
    response
}

#[test]
fn listener_answers_404_to_stray_requests_and_keeps_waiting_for_the_callback() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let browser = thread::spawn(move || {
        let favicon = send(addr, "GET /favicon.ico HTTP/1.1");
        let callback = send(addr, "GET /callback?code=c1&state=s1 HTTP/1.1");
        (favicon, callback)
    });

    let params = wait_for_callback(&listener, "s1").unwrap();
    let (favicon, callback) = browser.join().unwrap();

    assert_eq!(params, CallbackParams { code: "c1".to_string(), state: "s1".to_string() });
    assert!(favicon.starts_with("HTTP/1.1 404"), "got {favicon}");
    assert!(callback.starts_with("HTTP/1.1 200"), "got {callback}");
}

#[test]
fn listener_rejects_a_state_mismatch() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let browser = thread::spawn(move || send(addr, "GET /callback?code=c1&state=forged HTTP/1.1"));

    let err = wait_for_callback(&listener, "expected").unwrap_err();
    browser.join().unwrap();

    assert!(matches!(err, LoginError::StateMismatch), "got {err:?}");
}

#[test]
fn listener_reports_a_denied_consent() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let browser =
        thread::spawn(move || send(addr, "GET /callback?error=access_denied&state=s1 HTTP/1.1"));

    let err = wait_for_callback(&listener, "s1").unwrap_err();
    browser.join().unwrap();

    assert!(
        matches!(&err, LoginError::Callback(CallbackError::Denied(d)) if d == "access_denied"),
        "got {err:?}"
    );
}

// ── code exchange and refresh ─────────────────────────────────────────────

#[test]
fn exchange_code_posts_pkce_form_and_keeps_refresh_token() {
    let (url, server) = one_shot_server(
        "200 OK",
        r#"{"access_token":"at","refresh_token":"rt","expires_in":43199,"token_type":"Bearer"}"#,
    );

    let creds = exchange_code(&native_config(&url), "code-1", "verifier-1").unwrap();

    let request = server.join().unwrap();
    assert!(request.starts_with("POST /oauth/v2/token "), "got {request}");
    assert_eq!(
        form(&request),
        HashMap::from([
            ("grant_type".to_string(), "authorization_code".to_string()),
            ("code".to_string(), "code-1".to_string()),
            ("redirect_uri".to_string(), "http://localhost:8080/callback".to_string()),
            ("client_id".to_string(), "123@zitadel-cli".to_string()),
            ("code_verifier".to_string(), "verifier-1".to_string()),
        ])
    );
    assert_eq!(creds.access_token, "at");
    assert_eq!(creds.refresh_token.as_deref(), Some("rt"));
}

#[test]
fn exchange_without_refresh_token_explains_how_to_enable_it() {
    let (url, server) =
        one_shot_server("200 OK", r#"{"access_token":"at","expires_in":43199,"token_type":"Bearer"}"#);

    let err = exchange_code(&native_config(&url), "c", "v").unwrap_err();
    server.join().unwrap();

    assert!(matches!(err, LoginError::NoRefreshToken), "got {err:?}");
    assert_eq!(
        err.to_string(),
        "ZITADEL issued no refresh token, so the session could not be renewed. In the console enable \
        \"Refresh Token\" in the Native application's token settings, then run: zitadel auth login --user"
    );
}

fn user_credentials() -> Credentials {
    Credentials {
        access_token: "old".to_string(),
        refresh_token: Some("rt-old".to_string()),
        expires_at: 0,
    }
}

#[test]
fn refresh_posts_refresh_grant_and_rotates_refresh_token() {
    let (url, server) = one_shot_server(
        "200 OK",
        r#"{"access_token":"new","refresh_token":"rt-new","expires_in":43199}"#,
    );

    let creds = refresh(&native_config(&url), &user_credentials()).unwrap();

    assert_eq!(
        form(&server.join().unwrap()),
        HashMap::from([
            ("grant_type".to_string(), "refresh_token".to_string()),
            ("refresh_token".to_string(), "rt-old".to_string()),
            ("client_id".to_string(), "123@zitadel-cli".to_string()),
        ])
    );
    assert_eq!(creds.access_token, "new");
    assert_eq!(creds.refresh_token.as_deref(), Some("rt-new"));
}

#[test]
fn refresh_keeps_the_old_refresh_token_when_none_is_returned() {
    let (url, server) = one_shot_server("200 OK", r#"{"access_token":"new","expires_in":43199}"#);

    let creds = refresh(&native_config(&url), &user_credentials()).unwrap();
    server.join().unwrap();

    assert_eq!(creds.refresh_token.as_deref(), Some("rt-old"));
}

#[test]
fn expired_user_session_is_renewed_with_its_refresh_token_not_the_service_user() {
    // Regression: renew() used to always re-sign the service-user JWT, which
    // would have silently turned an expired human session into the service user.
    let (url, server) = one_shot_server(
        "200 OK",
        r#"{"access_token":"new","refresh_token":"rt-new","expires_in":43199}"#,
    );
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("credentials.json");
    save_credentials(&path, &user_credentials()).unwrap();

    let creds = load_credentials(&native_config(&url), &path).unwrap();

    assert_eq!(form(&server.join().unwrap())["grant_type"], "refresh_token");
    assert_eq!(creds.refresh_token.as_deref(), Some("rt-new"));
}
