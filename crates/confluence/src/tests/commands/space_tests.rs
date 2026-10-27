#![allow(clippy::unwrap_used, clippy::expect_used)]
//! `space get` (issue #257): v2 reads a space by key only as a filter on the list.

use serde_json::json;

use super::get_space;
use crate::auth::Credentials;
use crate::client::ConfluenceClient;
use crate::error::CliError;
use crate::test_support::mock_server;

fn client(url: &str) -> ConfluenceClient {
    let credentials = Credentials {
        access_token: "at".to_string(),
        refresh_token: None,
        expires_at: u64::MAX,
        cloud_id: "cid".to_string(),
        site_url: None,
    };
    ConfluenceClient::new(&credentials).with_base_url(url)
}

fn request_line(request: &str) -> &str {
    request.lines().next().unwrap()
}

#[test]
fn get_space_filters_the_list_by_key_and_returns_the_space_object() {
    let space = json!({
        "id": "98765",
        "key": "ENG",
        "name": "Engineering",
        "type": "global",
        "status": "current",
        "homepageId": "11111"
    });
    let response = json!({"results": [space], "_links": {}});
    let (url, server) = mock_server(&[("200 OK", &response.to_string())]);

    let value = get_space(&client(&url), "ENG").unwrap();

    let requests = server.join().unwrap();
    assert_eq!(request_line(&requests[0]), "GET /wiki/api/v2/spaces?keys=ENG HTTP/1.1");
    assert_eq!(value, space);
}

#[test]
fn get_space_encodes_the_key_so_it_cannot_add_query_parameters() {
    let (url, server) = mock_server(&[("200 OK", r#"{"results":[{"key":"~abc"}]}"#)]);

    get_space(&client(&url), "~abc&limit=1").unwrap();

    let requests = server.join().unwrap();
    assert_eq!(request_line(&requests[0]), "GET /wiki/api/v2/spaces?keys=%7Eabc%26limit%3D1 HTTP/1.1");
}

#[test]
fn get_space_reports_a_missing_key_with_the_command_to_find_keys() {
    for body in [r#"{"results":[],"_links":{}}"#, "{}"] {
        let (url, server) = mock_server(&[("200 OK", body)]);

        let error = get_space(&client(&url), "NOPE").unwrap_err();

        server.join().unwrap();
        assert!(matches!(&error, CliError::SpaceNotFound { key } if key == "NOPE"), "{body}: {error:?}");
        assert_eq!(
            error.to_string(),
            "no space with key \"NOPE\". Find space keys with: confluence space list --select results.key,results.name"
        );
    }
}

#[test]
fn get_space_keeps_api_errors() {
    let (url, server) = mock_server(&[("403 Forbidden", r#"{"message":"no"}"#)]);

    let error = get_space(&client(&url), "ENG").unwrap_err();

    server.join().unwrap();
    assert!(matches!(error, CliError::ApiError { status: 403, .. }), "{error:?}");
}
