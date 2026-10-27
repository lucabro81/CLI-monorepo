#![allow(clippy::unwrap_used, clippy::expect_used)]

use serde_json::json;

use super::{get_page, markdown_body, parse_body_source, update_page, validate_update_target, BodySource};
use crate::auth::Credentials;
use crate::cli::BodyFormat;
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

fn request_body(request: &str) -> serde_json::Value {
    serde_json::from_str(request.split("\r\n\r\n").nth(1).unwrap()).unwrap()
}

#[test]
fn parse_body_source_accepts_body_only() {
    let source = parse_body_source(Some("<p>hi</p>".to_string()), None, None).expect("should parse");

    assert_eq!(source, BodySource::Body("<p>hi</p>".to_string()));
}

#[test]
fn parse_body_source_accepts_body_file_only() {
    let source =
        parse_body_source(None, Some("./content.html".to_string()), None).expect("should parse");

    assert_eq!(source, BodySource::BodyFile("./content.html".to_string()));
}

#[test]
fn parse_body_source_accepts_template_id_only() {
    let source = parse_body_source(None, None, Some("4321".to_string())).expect("should parse");

    assert_eq!(source, BodySource::TemplateId("4321".to_string()));
}

#[test]
fn parse_body_source_rejects_none_provided() {
    let result = parse_body_source(None, None, None);

    assert!(matches!(result, Err(CliError::PageCreateMissingBodySource)));
}

#[test]
fn parse_body_source_rejects_more_than_one_as_internal_error() {
    // Unreachable in practice — clap's conflicts_with_all rules this out at
    // parse time — but the function must still fail loudly rather than
    // silently picking one, in case that invariant is ever broken.
    let result = parse_body_source(
        Some("<p>hi</p>".to_string()),
        Some("./content.html".to_string()),
        None,
    );

    assert!(matches!(result, Err(CliError::Internal(_))));
}

#[test]
fn validate_update_target_ok_with_title_only() {
    assert!(validate_update_target(Some("New title"), None).is_ok());
}

#[test]
fn validate_update_target_ok_with_body_only() {
    assert!(validate_update_target(None, Some("<p>new</p>")).is_ok());
}

#[test]
fn validate_update_target_ok_with_both() {
    assert!(validate_update_target(Some("New title"), Some("<p>new</p>")).is_ok());
}

#[test]
fn validate_update_target_err_with_neither() {
    let result = validate_update_target(None, None);

    assert!(matches!(result, Err(CliError::PageUpdateMissingTarget)));
}

#[test]
fn page_get_as_markdown_requests_export_view_and_replaces_it_with_markdown() {
    let page = json!({
        "id": "123",
        "title": "Doc",
        "version": {"number": 4},
        "body": {"export_view": {"representation": "export_view", "value": "<h1>Doc</h1><p>Hello <strong>you</strong></p>"}},
    });
    let (url, server) = mock_server(&[("200 OK", &page.to_string())]);

    let value = get_page(&client(&url), "123", BodyFormat::Markdown).unwrap();

    let requests = server.join().unwrap();
    assert_eq!(request_line(&requests[0]), "GET /wiki/api/v2/pages/123?body-format=export_view HTTP/1.1");
    assert_eq!(
        value,
        json!({
            "id": "123",
            "title": "Doc",
            "version": {"number": 4},
            "body": {"markdown": {"representation": "markdown", "value": "# Doc\n\nHello **you**"}},
        })
    );
}

#[test]
fn page_get_as_storage_requests_storage_and_prints_the_page_unchanged() {
    let page = json!({"id": "123", "body": {"storage": {"representation": "storage", "value": "<p>x</p>"}}});
    let (url, server) = mock_server(&[("200 OK", &page.to_string())]);

    let value = get_page(&client(&url), "123", BodyFormat::Storage).unwrap();

    let requests = server.join().unwrap();
    assert_eq!(request_line(&requests[0]), "GET /wiki/api/v2/pages/123?body-format=storage HTTP/1.1");
    assert_eq!(value, page);
}

#[test]
fn markdown_body_leaves_a_page_without_body_unchanged() {
    let page = json!({"id": "123", "title": "Doc"});

    assert_eq!(markdown_body(page.clone()).unwrap(), page);
}

#[test]
fn page_update_reads_and_writes_storage_even_though_page_get_defaults_to_markdown() {
    // Guards #254: `page get` prints Markdown by default; `page update` must keep
    // reading storage, or it would write Markdown back into the page.
    let current = json!({
        "id": "123",
        "title": "Doc",
        "version": {"number": 4},
        "body": {"storage": {"representation": "storage", "value": "<p>kept</p>"}},
    });
    let (url, server) = mock_server(&[("200 OK", &current.to_string()), ("200 OK", r#"{"id":"123"}"#)]);

    let value = update_page(&client(&url), "123", Some("New title"), None).unwrap();

    let requests = server.join().unwrap();
    assert_eq!(value, json!({"id": "123"}));
    assert_eq!(request_line(&requests[0]), "GET /wiki/api/v2/pages/123?body-format=storage HTTP/1.1");
    assert_eq!(request_line(&requests[1]), "PUT /wiki/api/v2/pages/123 HTTP/1.1");
    assert_eq!(
        request_body(&requests[1]),
        json!({
            "id": "123",
            "status": "current",
            "title": "New title",
            "body": {"representation": "storage", "value": "<p>kept</p>"},
            "version": {"number": 5},
        })
    );
}
