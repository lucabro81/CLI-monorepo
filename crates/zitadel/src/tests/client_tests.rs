#![allow(clippy::unwrap_used, clippy::expect_used)]

use super::{ClientError, ZitadelClient};
use crate::auth::Credentials;
use crate::test_support::one_shot_server;

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
        ClientError::Request(reason) => panic!("expected Status, got Request({reason})"),
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
