#![allow(clippy::unwrap_used, clippy::expect_used)]
//! Tests for the human (`auth login --user`) flow: authorization URL, callback
//! error wrapping, code exchange, refresh, and renewal dispatch. PKCE and the
//! loopback listener live in crates/oauth-user-login.

use std::collections::HashMap;

use oauth_user_login::{WaitError, bind_listener};

use super::{
    AppConfig, Credentials, LoginError, authorization_url, complete_remote_login, exchange_code,
    load_credentials, pending_login_path, refresh, save_credentials, start_remote_login,
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

// ── authorization URL ─────────────────────────────────────────────────────

#[test]
fn authorization_url_carries_pkce_scopes_and_redirect() {
    let url = authorization_url(&native_config("https://acme.zitadel.cloud"), "chal", "st", "http://localhost:8080/callback").unwrap();

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

    let err = authorization_url(&config, "c", "s", "http://localhost:8080/callback").unwrap_err();

    assert!(matches!(err, LoginError::NativeAppNotConfigured), "got {err:?}");
    assert_eq!(
        err.to_string(),
        "no Native app configured: app.json has no \"client_id\". Create a Native application \
        (PKCE, redirect http://localhost:8080/callback, refresh token enabled) in a ZITADEL project \
        and run: zitadel init --client-id <client-id>"
    );
}

// ── callback errors ───────────────────────────────────────────────────────
// Parsing and the listener loop are tested in crates/oauth-user-login. The
// retry command is added once, by CliError::UserLoginFailed.

// Regression: the retry command appeared twice, once from LoginError and once
// from CliError::UserLoginFailed.
#[test]
fn callback_errors_carry_the_library_message_unchanged() {
    let err = LoginError::Callback(WaitError::StateMismatch);

    assert_eq!(err.to_string(), WaitError::StateMismatch.to_string());
}

#[test]
fn a_busy_callback_port_carries_the_library_message_unchanged() {
    let busy = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let address = busy.local_addr().unwrap().to_string();
    let listener_err = bind_listener(&address).unwrap_err();
    let expected = listener_err.to_string();

    assert_eq!(LoginError::CallbackListener(listener_err).to_string(), expected);
}

// ── code exchange and refresh ─────────────────────────────────────────────

#[test]
fn exchange_code_posts_pkce_form_and_keeps_refresh_token() {
    let (url, server) = one_shot_server(
        "200 OK",
        r#"{"access_token":"at","refresh_token":"rt","expires_in":43199,"token_type":"Bearer"}"#,
    );

    let creds = exchange_code(&native_config(&url), "code-1", "verifier-1", "http://localhost:8080/callback").unwrap();

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

    let err = exchange_code(&native_config(&url), "c", "v", "http://localhost:8080/callback").unwrap_err();
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

// Guard: the listener only accepts requests on CALLBACK_PATH at
// CALLBACK_LISTEN_ADDR's port, so the redirect URI sent to the provider must
// point exactly there, or the login gets a 404 / WrongPath instead of the code.
#[test]
fn redirect_uri_points_at_the_callback_listener() {
    let port = crate::endpoints::CALLBACK_LISTEN_ADDR.rsplit_once(':').unwrap().1;

    assert_eq!(crate::endpoints::REDIRECT_URI, format!("http://localhost:{port}{}", crate::endpoints::CALLBACK_PATH));
}

// ── remote (two-step) login ───────────────────────────────────────────────

const REMOTE_URI: &str = "https://mercury.example.com/oauth/callback";
const NOW: u64 = 1_800_000_000;

fn query(url: &str) -> HashMap<String, String> {
    serde_urlencoded::from_str(url.split_once('?').unwrap().1).unwrap()
}

#[test]
fn pending_login_lives_next_to_the_credentials() {
    assert_eq!(
        pending_login_path(std::path::Path::new("/cfg")),
        std::path::PathBuf::from("/cfg/zitadel-cli/pending-login.json")
    );
}

#[test]
fn remote_start_saves_a_pending_login_and_builds_its_authorize_url() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("pending-login.json");

    let (url, pending) =
        start_remote_login(&native_config("https://acme.zitadel.cloud"), REMOTE_URI, &path, NOW).unwrap();

    let params = query(&url);
    assert!(url.starts_with("https://acme.zitadel.cloud/oauth/v2/authorize?"), "got {url}");
    assert_eq!(params["redirect_uri"], REMOTE_URI);
    assert_eq!(params["state"], pending.state);
    assert_eq!(params["code_challenge"], pending.code_challenge().unwrap());
    assert_eq!(params["code_challenge_method"], "S256");
    assert_eq!(pending.redirect_uri, REMOTE_URI);
    assert_eq!(pending.expires_at, NOW + 600);
    let on_disk: oauth_user_login::PendingLogin =
        serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    assert_eq!(on_disk, pending);
}

#[test]
fn remote_start_without_a_native_app_saves_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("pending-login.json");
    let config = AppConfig { client_id: None, ..native_config("https://acme.zitadel.cloud") };

    let err = start_remote_login(&config, REMOTE_URI, &path, NOW).unwrap_err();

    assert!(matches!(err, LoginError::NativeAppNotConfigured), "got {err:?}");
    assert!(!path.exists());
}

#[test]
fn remote_complete_exchanges_the_code_with_the_stored_verifier_and_redirect() {
    let (url, server) = one_shot_server(
        "200 OK",
        r#"{"access_token":"at","refresh_token":"rt","expires_in":43199,"token_type":"Bearer"}"#,
    );
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("pending-login.json");
    let (_, pending) = start_remote_login(&native_config(&url), REMOTE_URI, &path, NOW).unwrap();

    let creds = complete_remote_login(&native_config(&url), &path, "code-9", &pending.state, NOW + 30).unwrap();

    assert_eq!(
        form(&server.join().unwrap()),
        HashMap::from([
            ("grant_type".to_string(), "authorization_code".to_string()),
            ("code".to_string(), "code-9".to_string()),
            ("redirect_uri".to_string(), REMOTE_URI.to_string()),
            ("client_id".to_string(), "123@zitadel-cli".to_string()),
            ("code_verifier".to_string(), pending.code_verifier.clone().unwrap()),
        ])
    );
    assert_eq!(creds.refresh_token.as_deref(), Some("rt"));
    assert!(!path.exists(), "the pending login must be consumed");
}

#[test]
fn remote_complete_with_a_wrong_state_makes_no_token_request() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("pending-login.json");
    // An unroutable instance: any token request would fail differently.
    let config = native_config("http://127.0.0.1:9");
    start_remote_login(&config, REMOTE_URI, &path, NOW).unwrap();

    let err = complete_remote_login(&config, &path, "c", "stale", NOW).unwrap_err();

    assert!(
        matches!(err, LoginError::PendingLogin(oauth_user_login::PendingLoginError::StateMismatch)),
        "got {err:?}"
    );
    assert!(path.exists());
}

#[test]
fn remote_complete_consumes_the_state_even_when_the_provider_refuses_the_code() {
    let (url, server) = one_shot_server("400 Bad Request", r#"{"error":"invalid_grant"}"#);
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("pending-login.json");
    let (_, pending) = start_remote_login(&native_config(&url), REMOTE_URI, &path, NOW).unwrap();

    let err = complete_remote_login(&native_config(&url), &path, "used", &pending.state, NOW).unwrap_err();
    server.join().unwrap();

    assert!(matches!(err, LoginError::TokenExchange(_)), "got {err:?}");
    assert!(!path.exists());
}
