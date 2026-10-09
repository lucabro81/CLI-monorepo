#![allow(clippy::unwrap_used, clippy::expect_used)]
//! Issue #240: every request goes through `send`, which renews a token
//! answered 401 once and repeats the request with the new one.

use std::cell::RefCell;
use std::rc::Rc;

use super::{ClientError, JiraClient};
use crate::auth::Credentials;
use crate::error::CliError;
use crate::test_support::mock_server;

const REVOKED: &str = r#"{"error":"token revoked"}"#;

fn credentials() -> Credentials {
    Credentials {
        access_token: "at-revoked".to_string(),
        refresh_token: Some("rt".to_string()),
        expires_at: u64::MAX,
        cloud_id: "cid".to_string(),
        site_url: None,
    }
}

/// A client against `url` whose renewer records the rejected tokens and answers `result` once.
fn client(url: &str, result: Result<&str, CliError>) -> (JiraClient, Rc<RefCell<Vec<String>>>) {
    let rejected = Rc::new(RefCell::new(vec![]));
    let log = Rc::clone(&rejected);
    let result = RefCell::new(Some(result.map(str::to_string)));
    let client = JiraClient::new(&credentials()).with_base_url(url).with_renewer(Box::new(move |token: &str| {
        log.borrow_mut().push(token.to_string());
        result.borrow_mut().take().expect("renewed twice")
    }));
    (client, rejected)
}

fn bearer(request: &str) -> String {
    let lower = request.to_ascii_lowercase();
    let start = lower.find("authorization: bearer ").unwrap() + "authorization: bearer ".len();
    request[start..].lines().next().unwrap().to_string()
}

fn expired_login() -> CliError {
    CliError::UserLoginExpired { reason: "400: invalid_grant".to_string(), id: "alice".to_string() }
}

#[test]
fn a_401_renews_once_and_repeats_a_get_with_the_new_token() {
    let (url, server) = mock_server(&[("401 Unauthorized", REVOKED), ("200 OK", r#"{"ok":true}"#)]);
    let (client, rejected) = client(&url, Ok("at-fresh"));

    let value = client.get_myself().unwrap();

    let requests = server.join().unwrap();
    assert_eq!(value, serde_json::json!({"ok": true}));
    assert_eq!(*rejected.borrow(), ["at-revoked"]);
    assert_eq!((bearer(&requests[0]), bearer(&requests[1])), ("at-revoked".to_string(), "at-fresh".to_string()));
    assert_eq!(requests[0].lines().next(), requests[1].lines().next());
    assert_eq!(client.access_token(), "at-fresh");
}

#[test]
fn a_retried_post_sends_the_same_body_again() {
    let (url, server) = mock_server(&[("401 Unauthorized", REVOKED), ("200 OK", "{}")]);
    let (client, _) = client(&url, Ok("at-fresh"));

    client.apply_transition("KEY-1", "31").unwrap();

    let requests = server.join().unwrap();
    assert!(requests[1].starts_with("POST "), "got {}", requests[1]);
    assert_eq!(requests[0].split("\r\n\r\n").nth(1), requests[1].split("\r\n\r\n").nth(1));
    assert!(requests[1].ends_with(r#"{"transition":{"id":"31"}}"#), "got {}", requests[1]);
}

#[test]
fn a_retried_delete_succeeds_with_the_new_token() {
    let (url, server) = mock_server(&[("401 Unauthorized", REVOKED), ("204 No Content", "")]);
    let (client, _) = client(&url, Ok("at-fresh"));

    client.delete_comment("KEY-1", "10").unwrap();

    let requests = server.join().unwrap();
    assert!(requests[1].starts_with("DELETE "), "got {}", requests[1]);
    assert_eq!(bearer(&requests[1]), "at-fresh");
}

#[test]
fn a_second_401_is_returned_as_a_status_error_without_renewing_again() {
    let (url, server) = mock_server(&[("401 Unauthorized", REVOKED), ("401 Unauthorized", REVOKED)]);
    let (client, rejected) = client(&url, Ok("at-fresh"));

    let err = client.get_myself().unwrap_err();

    server.join().unwrap();
    assert!(matches!(err, ClientError::Status { status: 401, ref body } if body == REVOKED), "got {err:?}");
    assert_eq!(rejected.borrow().len(), 1);
}

#[test]
fn a_refused_renewal_is_returned_as_the_renewal_error() {
    // Issue #240: the person ended their session, so the refresh is refused
    // too; the error must reach the user as UserLoginExpired (exit 3).
    let (url, server) = mock_server(&[("401 Unauthorized", REVOKED)]);
    let (client, _) = client(&url, Err(expired_login()));

    let err = client.get_myself().unwrap_err();

    server.join().unwrap();
    match err {
        ClientError::Renewal(e) => assert!(matches!(*e, CliError::UserLoginExpired { ref id, .. } if id == "alice")),
        other => panic!("expected Renewal, got {other:?}"),
    }
}

#[test]
fn other_errors_do_not_renew() {
    let (url, server) = mock_server(&[("403 Forbidden", "{}")]);
    let (client, rejected) = client(&url, Ok("unused"));

    let err = client.get_myself().unwrap_err();

    server.join().unwrap();
    assert!(matches!(err, ClientError::Status { status: 403, .. }), "got {err:?}");
    assert!(rejected.borrow().is_empty());
}

#[test]
fn without_a_renewer_a_401_is_returned_as_is() {
    let (url, server) = mock_server(&[("401 Unauthorized", REVOKED)]);
    let client = JiraClient::new(&credentials()).with_base_url(&url);

    let err = client.get_myself().unwrap_err();

    server.join().unwrap();
    assert!(matches!(err, ClientError::Status { status: 401, .. }), "got {err:?}");
}
