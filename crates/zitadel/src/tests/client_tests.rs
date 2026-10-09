#![allow(clippy::unwrap_used, clippy::expect_used)]

use super::{ClientError, ZitadelClient};
use crate::auth::Credentials;
use crate::error::CliError;
use crate::test_support::{mock_server, one_shot_server};
use std::cell::RefCell;
use std::rc::Rc;

fn client(url: &str) -> ZitadelClient {
    ZitadelClient::new(
        url,
        &Credentials {
            access_token: "at-123".to_string(),
            refresh_token: None,
            expires_at: 0,
        },
    )
}

#[test]
fn get_current_user_sends_bearer_get_to_auth_users_me() {
    let (url, server) = one_shot_server("200 OK", r#"{"user":{"id":"42","userName":"bot"}}"#);

    let value = client(&url).get_current_user().unwrap();

    let request = server.join().unwrap();
    assert!(request.starts_with("GET /auth/v1/users/me "), "got {request}");
    let lower = request.to_ascii_lowercase();
    assert!(lower.contains("authorization: bearer at-123\r\n"), "got {request}");
    assert!(lower.contains("accept: application/json\r\n"), "got {request}");
    assert_eq!(value, serde_json::json!({"user": {"id": "42", "userName": "bot"}}));
}

#[test]
fn non_success_status_returns_status_and_body() {
    let (url, server) = one_shot_server("403 Forbidden", r#"{"code":7,"message":"No matching permissions found"}"#);

    let err = client(&url).get_current_user().unwrap_err();
    server.join().unwrap();

    match err {
        ClientError::Status { status, body } => {
            assert_eq!(status, 403);
            assert_eq!(body, r#"{"code":7,"message":"No matching permissions found"}"#);
        }
        other => panic!("expected Status, got {other:?}"),
    }
}

#[test]
fn unreachable_instance_returns_request_error() {
    // Bind then drop to get a local port nothing is listening on.
    let port = std::net::TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap().port();

    let err = client(&format!("http://127.0.0.1:{port}")).get_current_user().unwrap_err();

    assert!(matches!(err, ClientError::Request(_)), "got {err:?}");
}

#[test]
fn get_user_sends_get_to_v2_users_id() {
    let (url, server) = one_shot_server("200 OK", r#"{"user":{"userId":"42"}}"#);

    let value = client(&url).get_user("42").unwrap();

    assert!(server.join().unwrap().starts_with("GET /v2/users/42 "));
    assert_eq!(value, serde_json::json!({"user": {"userId": "42"}}));
}

#[test]
fn get_user_percent_encodes_the_id_as_a_single_path_segment() {
    // An id containing "/" must not turn into a different endpoint path.
    let (url, server) = one_shot_server("404 Not Found", r#"{"code":5,"message":"Not Found"}"#);

    let _ = client(&url).get_user("a/b c");

    let request = server.join().unwrap();
    assert!(request.starts_with("GET /v2/users/a%2Fb%20c "), "got {request}");
}

#[test]
fn list_organizations_posts_body_to_v2_organizations_search() {
    let (url, server) = one_shot_server("200 OK", r#"{"result":[]}"#);

    client(&url).list_organizations(&serde_json::json!({"queries": []})).unwrap();

    let request = server.join().unwrap();
    assert!(request.starts_with("POST /v2/organizations/_search "), "got {request}");
    assert!(request.ends_with(r#"{"queries":[]}"#), "got {request}");
}

#[test]
fn list_projects_posts_json_to_the_connect_project_service() {
    // The v2 ProjectService has no REST mapping: it is only reachable through the
    // Connect protocol path, with a plain application/json body.
    let (url, server) = one_shot_server("200 OK", r#"{"projects":[]}"#);

    client(&url).list_projects(&serde_json::json!({"filters": []})).unwrap();

    let request = server.join().unwrap();
    assert!(
        request.starts_with("POST /zitadel.project.v2.ProjectService/ListProjects "),
        "got {request}"
    );
    assert!(request.to_ascii_lowercase().contains("content-type: application/json\r\n"), "got {request}");
    assert!(request.ends_with(r#"{"filters":[]}"#), "got {request}");
}

#[test]
fn search_users_posts_body_to_v2_users() {
    let (url, server) = one_shot_server("200 OK", r#"{"result":[]}"#);

    client(&url).search_users(&serde_json::json!({"queries": []})).unwrap();

    let request = server.join().unwrap();
    assert!(request.starts_with("POST /v2/users "), "got {request}");
    assert!(request.ends_with(r#"{"queries":[]}"#), "got {request}");
}

#[test]
fn success_status_with_non_json_body_is_a_request_error() {
    // e.g. a proxy or login page answering 200 with HTML.
    let (url, server) = one_shot_server("200 OK", "<html>login</html>");

    let err = client(&url).get_current_user().unwrap_err();
    server.join().unwrap();

    assert!(
        matches!(&err, ClientError::Request(reason) if reason.starts_with("invalid JSON response:")),
        "got {err:?}"
    );
}

// ── renewal after a 401 (issue #240) ──────────────────────────────────────

const REVOKED: &str = r#"{"code":16,"message":"Errors.Token.Invalid (AUTH-7fs1e)"}"#;

/// A client whose renewer records the rejected tokens it is given and answers `result`.
fn renewing_client(url: &str, result: Result<&str, CliError>) -> (ZitadelClient, Rc<RefCell<Vec<String>>>) {
    let calls = Rc::new(RefCell::new(vec![]));
    let seen = Rc::clone(&calls);
    let result = RefCell::new(Some(result.map(str::to_string)));
    let client = client(url).with_renewer(Box::new(move |rejected: &str| {
        seen.borrow_mut().push(rejected.to_string());
        result.borrow_mut().take().expect("renewer called twice")
    }));
    (client, calls)
}

fn bearer(request: &str) -> String {
    let lower = request.to_ascii_lowercase();
    let start = lower.find("authorization: bearer ").unwrap() + "authorization: bearer ".len();
    request[start..].lines().next().unwrap().to_string()
}

#[test]
fn a_401_renews_the_token_once_and_repeats_the_request_with_the_new_one() {
    // Issue #240: a revoked (not expired) token was sent once and the 401
    // reported as is; the client must renew and retry.
    let (url, server) = mock_server(&[("401 Unauthorized", REVOKED), ("200 OK", r#"{"user":{"id":"42"}}"#)]);
    let (client, calls) = renewing_client(&url, Ok("at-fresh"));

    let value = client.get_current_user().unwrap();

    let requests = server.join().unwrap();
    assert_eq!(value, serde_json::json!({"user": {"id": "42"}}));
    assert_eq!(*calls.borrow(), vec!["at-123".to_string()]);
    assert_eq!((bearer(&requests[0]), bearer(&requests[1])), ("at-123".to_string(), "at-fresh".to_string()));
    assert!(requests[1].starts_with("GET /auth/v1/users/me "), "got {}", requests[1]);
}

#[test]
fn a_retried_post_sends_the_same_body_again() {
    let body = serde_json::json!({"query": {"limit": 1}});
    let (url, server) = mock_server(&[("401 Unauthorized", REVOKED), ("200 OK", "{}")]);
    let (client, _) = renewing_client(&url, Ok("at-fresh"));

    client.search_users(&body).unwrap();

    let requests = server.join().unwrap();
    assert!(requests[1].starts_with("POST /v2/users "), "got {}", requests[1]);
    assert!(requests[1].ends_with(&body.to_string()), "got {}", requests[1]);
}

#[test]
fn a_second_401_with_the_renewed_token_is_returned_without_renewing_again() {
    let (url, server) = mock_server(&[("401 Unauthorized", REVOKED), ("401 Unauthorized", REVOKED)]);
    let (client, calls) = renewing_client(&url, Ok("at-fresh"));

    let err = client.get_current_user().unwrap_err();

    server.join().unwrap();
    assert!(matches!(err, ClientError::Status { status: 401, ref body } if body == REVOKED), "got {err:?}");
    assert_eq!(calls.borrow().len(), 1);
}

#[test]
fn a_failed_renewal_after_a_401_is_returned_as_the_renewal_error() {
    let (url, server) = mock_server(&[("401 Unauthorized", REVOKED)]);
    let refused = CliError::UserLoginExpired { reason: "400: invalid_grant".to_string(), id: "alice".to_string() };
    let (client, _) = renewing_client(&url, Err(refused));

    let err = client.get_current_user().unwrap_err();

    server.join().unwrap();
    match err {
        ClientError::Renewal(e) => assert!(matches!(*e, CliError::UserLoginExpired { ref id, .. } if id == "alice")),
        other => panic!("expected Renewal, got {other:?}"),
    }
}

#[test]
fn a_renewed_token_is_kept_for_the_next_requests() {
    let (url, server) = mock_server(&[("401 Unauthorized", REVOKED), ("200 OK", "{}"), ("200 OK", "{}")]);
    let (client, calls) = renewing_client(&url, Ok("at-fresh"));

    client.get_current_user().unwrap();
    client.list_my_memberships().unwrap();

    let requests = server.join().unwrap();
    assert_eq!(bearer(&requests[2]), "at-fresh");
    assert_eq!(calls.borrow().len(), 1);
}

#[test]
fn other_errors_do_not_renew() {
    let (url, server) = mock_server(&[("403 Forbidden", "{}")]);
    let (client, calls) = renewing_client(&url, Ok("unused"));

    let err = client.get_current_user().unwrap_err();

    server.join().unwrap();
    assert!(matches!(err, ClientError::Status { status: 403, .. }), "got {err:?}");
    assert!(calls.borrow().is_empty());
}

#[test]
fn without_a_renewer_a_401_is_returned_as_is() {
    let (url, server) = one_shot_server("401 Unauthorized", REVOKED);

    let err = client(&url).get_current_user().unwrap_err();

    server.join().unwrap();
    assert!(matches!(err, ClientError::Status { status: 401, .. }), "got {err:?}");
}
