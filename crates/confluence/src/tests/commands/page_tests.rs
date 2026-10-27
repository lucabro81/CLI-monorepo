#![allow(clippy::unwrap_used, clippy::expect_used)]

use serde_json::json;

use super::{
    get_page, page_ancestors, page_children, page_comments, markdown_body, parse_body_source, search_pages, strip_highlight, update_page, validate_update_target,
    BodySource,
};
use crate::auth::Credentials;
use crate::cli::BodyFormat;
use crate::client::ConfluenceClient;
use crate::error::CliError;
use crate::test_support::mock_server;

fn client(url: &str) -> ConfluenceClient {
    client_for_site(url, None)
}

/// A client whose credentials carry `site_url` (accessible-resources' `url`).
fn client_for_site(url: &str, site_url: Option<&str>) -> ConfluenceClient {
    let credentials = Credentials {
        access_token: "at".to_string(),
        refresh_token: None,
        expires_at: u64::MAX,
        cloud_id: "cid".to_string(),
        site_url: site_url.map(str::to_string),
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
fn page_get_as_markdown_makes_wiki_links_absolute_with_the_credentials_site_url() {
    // Issue #261.
    let page = json!({"id": "123", "body": {"export_view": {"value": r#"<p><a href="/wiki/x">x</a></p>"#}}});
    let (url, server) = mock_server(&[("200 OK", &page.to_string())]);

    let value = get_page(&client_for_site(&url, Some("https://example.atlassian.net")), "123", BodyFormat::Markdown).unwrap();

    server.join().unwrap();
    assert_eq!(value["body"]["markdown"]["value"], "[x](https://example.atlassian.net/wiki/x)");
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

    assert_eq!(markdown_body(page.clone(), None).unwrap(), page);
}

#[test]
fn markdown_body_without_a_string_export_view_value_prints_an_empty_markdown_body() {
    for body in [json!({}), json!({"export_view": {"value": null}})] {
        let page = json!({"id": "123", "body": body});

        assert_eq!(
            markdown_body(page, None).unwrap(),
            json!({"id": "123", "body": {"markdown": {"representation": "markdown", "value": ""}}})
        );
    }
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

// --- page search (issue #255) ---

#[test]
fn search_pages_asks_v1_search_with_highlighted_excerpts_and_strips_the_markers_from_excerpt_and_titles() {
    let response = json!({
        "results": [
            {
                "content": {"id": "11", "type": "page", "title": "Runbook"},
                "title": "@@@hl@@@Runbook@@@endhl@@@",
                "excerpt": "restart the @@@hl@@@api@@@endhl@@@ with @@@hl@@@care@@@endhl@@@",
                "url": "/spaces/ENG/pages/11/Runbook",
                "resultGlobalContainer": {"title": "@@@hl@@@Eng@@@endhl@@@ineering", "displayUrl": "/spaces/ENG"},
                "lastModified": "2026-10-01T10:00:00.000Z"
            },
            {"content": {"id": "12", "type": "page", "title": "Plain"}, "excerpt": "no markers here"}
        ],
        "start": 25,
        "limit": 10,
        "size": 2,
        "_links": {"base": "https://example.atlassian.net/wiki"}
    });
    let (url, server) = mock_server(&[("200 OK", &response.to_string())]);

    let value = search_pages(&client(&url), "type=page AND space=ENG", 10, 25).unwrap();

    let requests = server.join().unwrap();
    assert_eq!(
        request_line(&requests[0]),
        "GET /wiki/rest/api/search?cql=type%3Dpage+AND+space%3DENG&limit=10&start=25&excerpt=highlight_unescaped HTTP/1.1"
    );
    let mut expected = response;
    expected["results"][0]["excerpt"] = json!("restart the api with care");
    expected["results"][0]["title"] = json!("Runbook");
    expected["results"][0]["resultGlobalContainer"]["title"] = json!("Engineering");
    assert_eq!(value, expected);
}

#[test]
fn search_pages_leaves_results_without_a_string_excerpt_unchanged() {
    let response = json!({
        "results": [
            {"content": {"id": "11"}},
            {"content": {"id": "12"}, "excerpt": null}
        ],
        "size": 2
    });
    let (url, server) = mock_server(&[("200 OK", &response.to_string())]);

    let value = search_pages(&client(&url), "type=page", 25, 0).unwrap();

    server.join().unwrap();
    assert_eq!(value, response);
}

#[test]
fn search_pages_reports_a_400_as_invalid_cql_with_confluence_message() {
    let body = r#"{"statusCode":400,"message":"Could not parse cql : type=pag AND"}"#;
    let (url, server) = mock_server(&[("400 Bad Request", body)]);

    let error = search_pages(&client(&url), "type=pag AND", 25, 0).unwrap_err();

    server.join().unwrap();
    assert!(
        matches!(&error, CliError::InvalidCql { cql, body: b } if cql == "type=pag AND" && b == body),
        "{error:?}"
    );
    let message = error.to_string();
    assert!(message.contains("Could not parse cql : type=pag AND"), "{message}");
    assert!(message.contains("confluence page search --cql"), "{message}");
}

#[test]
fn search_pages_keeps_other_statuses_as_api_errors() {
    let (url, server) = mock_server(&[("403 Forbidden", r#"{"message":"no"}"#)]);

    let error = search_pages(&client(&url), "type=page", 25, 0).unwrap_err();

    server.join().unwrap();
    assert!(matches!(error, CliError::ApiError { status: 403, .. }), "{error:?}");
}

#[test]
fn strip_highlight_removes_every_marker_and_keeps_the_text() {
    assert_eq!(strip_highlight(""), "");
    assert_eq!(strip_highlight("plain text"), "plain text");
    assert_eq!(strip_highlight("@@@hl@@@a@@@endhl@@@ b @@@hl@@@c@@@endhl@@@"), "a b c");
    // An unclosed marker (excerpt cut at its length limit) still leaves clean text.
    assert_eq!(strip_highlight("cut @@@hl@@@wor"), "cut wor");
}

// --- page children / page ancestors (issue #256) ---

#[test]
fn page_children_asks_direct_children_and_prints_the_response() {
    let response = json!({
        "results": [
            {"id": "21", "title": "Child", "type": "page", "status": "current", "spaceId": "9", "childPosition": 0},
            {"id": "22", "title": "Folder", "type": "folder", "status": "current", "spaceId": "9", "childPosition": 1}
        ],
        "_links": {"next": "/wiki/api/v2/pages/123/direct-children?cursor=xyz"}
    });
    let (url, server) = mock_server(&[("200 OK", &response.to_string())]);

    let value = page_children(&client(&url), "123", 25, None).unwrap();

    let requests = server.join().unwrap();
    assert_eq!(request_line(&requests[0]), "GET /wiki/api/v2/pages/123/direct-children?limit=25 HTTP/1.1");
    let mut expected = response;
    expected["nextCursor"] = json!("xyz");
    assert_eq!(value, expected);
}

#[test]
fn page_children_links_page_children_only_when_the_site_is_known() {
    // Issue #271: children carry no space key, so the link goes through viewpage.action.
    let response = json!({
        "results": [
            {"id": "21", "title": "Child", "type": "page"},
            {"id": "22", "title": "Folder", "type": "folder"},
            {"id": "23", "title": "Board", "type": "whiteboard"}
        ]
    });
    for site_url in [Some("https://example.atlassian.net/"), None] {
        let (url, server) = mock_server(&[("200 OK", &response.to_string())]);

        let value = page_children(&client_for_site(&url, site_url), "123", 25, None).unwrap();

        server.join().unwrap();
        let mut expected = response.clone();
        if site_url.is_some() {
            expected["results"][0]["link"] = json!("https://example.atlassian.net/wiki/pages/viewpage.action?pageId=21");
        }
        assert_eq!(value, expected, "{site_url:?}");
    }
}

#[test]
fn page_children_passes_the_cursor() {
    let (url, server) = mock_server(&[("200 OK", r#"{"results":[]}"#)]);

    page_children(&client(&url), "123", 10, Some("xyz=")).unwrap();

    let requests = server.join().unwrap();
    assert_eq!(
        request_line(&requests[0]),
        "GET /wiki/api/v2/pages/123/direct-children?limit=10&cursor=xyz%3D HTTP/1.1"
    );
}

#[test]
fn a_404_on_children_or_ancestors_is_page_not_found() {
    for ancestors in [false, true] {
        let (url, server) = mock_server(&[("404 Not Found", r#"{"errors":[{"status":404}]}"#)]);

        let error = if ancestors {
            page_ancestors(&client(&url), "999").unwrap_err()
        } else {
            page_children(&client(&url), "999", 25, None).unwrap_err()
        };

        server.join().unwrap();
        assert!(matches!(&error, CliError::PageNotFound { id } if id == "999"), "{error:?}");
        assert_eq!(
            error.to_string(),
            "no page with id \"999\" that this identity can see (Confluence answered 404). Check the id, \
            e.g. with: confluence page search --cql \"type=page AND title~\\\"<words>\\\"\" \
            --select results.content.id,results.content.title"
        );
    }
}

#[test]
fn page_ancestors_adds_each_page_ancestors_title_and_leaves_folders_alone() {
    let ancestors = json!({
        "results": [
            {"id": "1", "type": "page"},
            {"id": "5", "type": "folder"},
            {"id": "2", "type": "page"}
        ]
    });
    let (url, server) = mock_server(&[
        ("200 OK", &ancestors.to_string()),
        ("200 OK", r#"{"id":"1","title":"Home","status":"current"}"#),
        ("200 OK", r#"{"id":"2","title":"Guides","status":"current"}"#),
    ]);

    let value = page_ancestors(&client(&url), "123").unwrap();

    let requests = server.join().unwrap();
    let lines: Vec<&str> = requests.iter().map(|r| request_line(r)).collect();
    assert_eq!(
        lines,
        [
            "GET /wiki/api/v2/pages/123/ancestors?limit=250 HTTP/1.1",
            "GET /wiki/api/v2/pages/1 HTTP/1.1",
            "GET /wiki/api/v2/pages/2 HTTP/1.1",
        ]
    );
    assert_eq!(
        value,
        json!({
            "results": [
                {"id": "1", "type": "page", "title": "Home"},
                {"id": "5", "type": "folder"},
                {"id": "2", "type": "page", "title": "Guides"}
            ]
        })
    );
}

#[test]
fn page_ancestors_links_page_ancestors_but_not_folders() {
    // Issue #271: same link as `page children`, also on an ancestor whose title lookup got a 404.
    let (url, server) = mock_server(&[
        ("200 OK", r#"{"results":[{"id":"1","type":"page"},{"id":"5","type":"folder"},{"id":"2","type":"page"}]}"#),
        ("200 OK", r#"{"id":"1","title":"Home"}"#),
        ("404 Not Found", r#"{"errors":[{"status":404}]}"#),
    ]);

    let value = page_ancestors(&client_for_site(&url, Some("https://example.atlassian.net")), "123").unwrap();

    server.join().unwrap();
    assert_eq!(
        value,
        json!({
            "results": [
                {"id": "1", "type": "page", "title": "Home", "link": "https://example.atlassian.net/wiki/pages/viewpage.action?pageId=1"},
                {"id": "5", "type": "folder"},
                {"id": "2", "type": "page", "link": "https://example.atlassian.net/wiki/pages/viewpage.action?pageId=2"}
            ]
        })
    );
}

#[test]
fn page_ancestors_of_a_top_level_page_is_an_empty_list() {
    let (url, server) = mock_server(&[("200 OK", r#"{"results":[]}"#)]);

    let value = page_ancestors(&client(&url), "123").unwrap();

    assert_eq!(server.join().unwrap().len(), 1);
    assert_eq!(value, json!({"results": []}));
}

#[test]
fn a_failed_ancestor_title_lookup_is_reported_not_skipped() {
    let (url, server) = mock_server(&[
        ("200 OK", r#"{"results":[{"id":"1","type":"page"}]}"#),
        ("403 Forbidden", r#"{"message":"no"}"#),
    ]);

    let error = page_ancestors(&client(&url), "123").unwrap_err();

    server.join().unwrap();
    assert!(matches!(error, CliError::ApiError { status: 403, .. }), "{error:?}");
}

#[test]
fn an_ancestor_page_this_identity_cannot_read_keeps_id_and_type_only() {
    let (url, server) = mock_server(&[
        ("200 OK", r#"{"results":[{"id":"1","type":"page"},{"id":"2","type":"page"}]}"#),
        ("404 Not Found", r#"{"errors":[{"status":404}]}"#),
        ("200 OK", r#"{"id":"2","title":"Guides"}"#),
    ]);

    let value = page_ancestors(&client(&url), "123").unwrap();

    server.join().unwrap();
    assert_eq!(
        value,
        json!({"results": [{"id": "1", "type": "page"}, {"id": "2", "type": "page", "title": "Guides"}]})
    );
}

#[test]
fn search_pages_adds_an_absolute_link_to_each_result_with_a_url() {
    // Issue #261: `url` is relative to the site's /wiki; `link` can be followed as is.
    let response = json!({
        "results": [
            {"content": {"id": "11"}, "url": "/spaces/ENG/pages/11/Runbook"},
            {"content": {"id": "12"}},
            {"content": {"id": "13"}, "url": null}
        ],
        "_links": {"base": "https://api.atlassian.com/ex/confluence/cid/wiki"}
    });
    let (url, server) = mock_server(&[("200 OK", &response.to_string())]);

    let value = search_pages(&client_for_site(&url, Some("https://example.atlassian.net/")), "type=page", 25, 0).unwrap();

    server.join().unwrap();
    let mut expected = response;
    expected["results"][0]["link"] = json!("https://example.atlassian.net/wiki/spaces/ENG/pages/11/Runbook");
    assert_eq!(value, expected);
}

#[test]
fn search_pages_adds_no_link_without_a_site_url() {
    let response = json!({"results": [{"content": {"id": "11"}, "url": "/spaces/ENG/pages/11/Runbook"}]});
    let (url, server) = mock_server(&[("200 OK", &response.to_string())]);

    let value = search_pages(&client(&url), "type=page", 25, 0).unwrap();

    server.join().unwrap();
    assert_eq!(value, response);
}

// --- page comments (issue #258) ---

#[test]
fn page_comments_adds_next_cursor_in_both_body_formats() {
    // Issue #271.
    let response = json!({"results": [], "_links": {"next": "/wiki/api/v2/pages/123/footer-comments?cursor=c%3D2&limit=5"}});
    for body_format in [BodyFormat::Markdown, BodyFormat::Storage] {
        let (url, server) = mock_server(&[("200 OK", &response.to_string())]);

        let value = page_comments(&client(&url), "123", 5, None, body_format).unwrap();

        server.join().unwrap();
        assert_eq!(value["nextCursor"], json!("c=2"), "{body_format:?}");
    }
}

#[test]
fn page_comments_as_markdown_reads_storage_and_replaces_each_body() {
    let response = json!({
        "results": [
            {
                "id": "c1",
                "status": "current",
                "pageId": "123",
                "version": {"number": 1, "authorId": "acc-1", "createdAt": "2026-10-01T10:00:00.000Z"},
                "body": {"storage": {"representation": "storage", "value": "<p>Use <code>v2</code>.</p>"}}
            },
            {"id": "c2", "body": {}}
        ],
        "_links": {"next": "/wiki/api/v2/pages/123/footer-comments?cursor=xyz"}
    });
    let (url, server) = mock_server(&[("200 OK", &response.to_string())]);

    let value = page_comments(&client(&url), "123", 25, None, BodyFormat::Markdown).unwrap();

    let requests = server.join().unwrap();
    assert_eq!(
        request_line(&requests[0]),
        "GET /wiki/api/v2/pages/123/footer-comments?body-format=storage&limit=25 HTTP/1.1"
    );
    let mut expected = response;
    expected["results"][0]["body"] = json!({"markdown": {"representation": "markdown", "value": "Use `v2`."}});
    expected["results"][1]["body"] = json!({"markdown": {"representation": "markdown", "value": ""}});
    expected["nextCursor"] = json!("xyz");
    assert_eq!(value, expected);
}

#[test]
fn page_comments_as_storage_prints_the_response_unchanged_and_passes_the_cursor() {
    let response = json!({"results": [{"id": "c1", "body": {"storage": {"value": "<p>x</p>"}}}]});
    let (url, server) = mock_server(&[("200 OK", &response.to_string())]);

    let value = page_comments(&client(&url), "123", 5, Some("xyz="), BodyFormat::Storage).unwrap();

    let requests = server.join().unwrap();
    assert_eq!(
        request_line(&requests[0]),
        "GET /wiki/api/v2/pages/123/footer-comments?body-format=storage&limit=5&cursor=xyz%3D HTTP/1.1"
    );
    assert_eq!(value, response);
}

#[test]
fn page_comments_on_a_missing_page_is_page_not_found() {
    let (url, server) = mock_server(&[("404 Not Found", r#"{"errors":[]}"#)]);

    let error = page_comments(&client(&url), "999", 25, None, BodyFormat::Markdown).unwrap_err();

    server.join().unwrap();
    assert!(matches!(&error, CliError::PageNotFound { id } if id == "999"), "{error:?}");
}

#[test]
fn page_comments_refused_403_names_the_comment_scope_and_how_to_get_it() {
    let (url, server) = mock_server(&[("403 Forbidden", r#"{"message":"scope does not match"}"#)]);

    let error = page_comments(&client(&url), "123", 25, None, BodyFormat::Markdown).unwrap_err();

    server.join().unwrap();
    assert!(matches!(&error, CliError::CommentsForbidden { body } if body.contains("scope does not match")), "{error:?}");
    let message = error.to_string();
    for needle in ["read:comment:confluence", "confluence auth login", "confluence auth login --user <USER_ID>"] {
        assert!(message.contains(needle), "{needle}: {message}");
    }
}

#[test]
fn page_comments_leaves_a_comment_without_body_unchanged_and_empties_a_non_string_value() {
    let response = json!({
        "results": [
            {"id": "c1"},
            {"id": "c2", "body": {"storage": {"value": 42}}}
        ]
    });
    let (url, server) = mock_server(&[("200 OK", &response.to_string())]);

    let value = page_comments(&client(&url), "123", 25, None, BodyFormat::Markdown).unwrap();

    server.join().unwrap();
    assert_eq!(
        value,
        json!({
            "results": [
                {"id": "c1"},
                {"id": "c2", "body": {"markdown": {"representation": "markdown", "value": ""}}}
            ]
        })
    );
}
