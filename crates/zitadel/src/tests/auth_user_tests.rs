#![allow(clippy::unwrap_used, clippy::expect_used)]
//! Tests for the human (`auth login --user`) flow: authorization URL, callback
//! error wrapping, code exchange, refresh, and renewal dispatch. PKCE and the
//! loopback listener live in crates/oauth-user-login.

use std::collections::HashMap;

use oauth_user_login::{WaitError, bind_listener};

use super::{
    AppConfig, Credentials, LoginError, authorization_url, exchange_code, load_credentials,
    refresh, save_credentials,
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

// Guard: the listener only accepts requests on CALLBACK_PATH at
// CALLBACK_LISTEN_ADDR's port, so the redirect URI sent to the provider must
// point exactly there, or the login gets a 404 / WrongPath instead of the code.
#[test]
fn redirect_uri_points_at_the_callback_listener() {
    let port = crate::endpoints::CALLBACK_LISTEN_ADDR.rsplit_once(':').unwrap().1;

    assert_eq!(crate::endpoints::REDIRECT_URI, format!("http://localhost:{port}{}", crate::endpoints::CALLBACK_PATH));
}
