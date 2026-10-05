#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::path::{Path, PathBuf};

use oauth_user_login::{CallbackError, WaitError};

use super::{
    app_config_path, authorization_code_body, authorization_url, complete_remote_login,
    complete_remote_login_at,
    credentials_path, merge_scopes_for_cloud_id, pending_login_path, refresh, start_remote_login,
    AccessibleResource, Credentials, LoginError, OAuthConfig, OAuthConfigError,
};

const TEST_SCOPES: &str = "read:example write:example offline_access";

#[test]
fn builds_authorization_url_with_required_params() {
    let config = OAuthConfig {
        client_id: "my-client-id".to_string(),
        client_secret: "shh".to_string(),
        redirect_uri: "http://localhost:8080/callback".to_string(),
    };

    let url = authorization_url(&config, "challenge123", "state456", TEST_SCOPES)
        .expect("should build URL");

    assert!(url.starts_with("https://auth.atlassian.com/authorize?"));
    assert!(url.contains("client_id=my-client-id"));
    assert!(url.contains("code_challenge=challenge123"));
    assert!(url.contains("code_challenge_method=S256"));
    assert!(url.contains("state=state456"));
    assert!(url.contains("response_type=code"));
    assert!(url.contains("audience=api.atlassian.com"));
    assert!(url.contains("redirect_uri=http%3A%2F%2Flocalhost%3A8080%2Fcallback"));
    assert!(url.contains("scope=read%3Aexample+write%3Aexample+offline_access"));
}

// PKCE and callback parsing are tested in crates/oauth-user-login.

// Regression for #70: a denied consent (`error=access_denied` on the callback)
// used to surface as a confusing "missing code" error.
#[test]
fn denied_consent_surfaces_the_providers_error() {
    let err = LoginError::Callback(WaitError::Callback(CallbackError::Denied {
        error: "access_denied".to_string(),
        description: Some("User did not authorize the request".to_string()),
    }));

    assert_eq!(
        err.to_string(),
        "authorization denied: access_denied (User did not authorize the request). Approve the \
        consent page to log in, then retry the login"
    );
}

#[test]
fn credentials_round_trip_through_json() {
    let creds = Credentials {
        access_token: "access".to_string(),
        refresh_token: Some("refresh".to_string()),
        expires_at: 1_700_000_000,
        cloud_id: "cloud-123".to_string(),
        site_url: Some("https://mysite.atlassian.net".to_string()),
    };

    let json = serde_json::to_string(&creds).unwrap();
    let parsed: Credentials = serde_json::from_str(&json).unwrap();

    assert_eq!(parsed, creds);
}

#[test]
fn credentials_round_trip_with_no_refresh_token() {
    // Service account (client_credentials) credentials have no refresh token.
    let creds = Credentials {
        access_token: "access".to_string(),
        refresh_token: None,
        expires_at: 1_700_000_000,
        cloud_id: "cloud-123".to_string(),
        site_url: Some("https://mysite.atlassian.net".to_string()),
    };

    let json = serde_json::to_string(&creds).unwrap();
    let parsed: Credentials = serde_json::from_str(&json).unwrap();

    assert_eq!(parsed, creds);
}

#[test]
fn credentials_without_refresh_token_field_deserializes_to_none() {
    // Forward/backward compatibility: a credentials.json missing the
    // refresh_token key entirely (e.g. hand-written for a service account)
    // must still parse, with refresh_token defaulting to None.
    let json = r#"{"access_token": "at", "expires_at": 1000, "cloud_id": "cid"}"#;

    let creds: Credentials = serde_json::from_str(json).unwrap();

    assert_eq!(creds.refresh_token, None);
}

#[test]
fn credentials_round_trip_with_no_site_url() {
    let creds = Credentials {
        access_token: "access".to_string(),
        refresh_token: Some("refresh".to_string()),
        expires_at: 1_700_000_000,
        cloud_id: "cloud-123".to_string(),
        site_url: None,
    };

    let json = serde_json::to_string(&creds).unwrap();
    let parsed: Credentials = serde_json::from_str(&json).unwrap();

    assert_eq!(parsed, creds);
}

#[test]
fn credentials_without_site_url_field_deserializes_to_none() {
    // Forward/backward compatibility: every credentials.json written before
    // this field existed has no site_url key at all — must still parse, with
    // site_url defaulting to None (browse_url is simply omitted downstream).
    let json = r#"{"access_token": "at", "expires_at": 1000, "cloud_id": "cid"}"#;

    let creds: Credentials = serde_json::from_str(json).unwrap();

    assert_eq!(creds.site_url, None);
}

#[test]
fn refresh_without_refresh_token_returns_internal_error() {
    // Service account credentials (refresh_token: None) cannot be renewed via
    // the refresh_token grant — refresh() must reject this before making any
    // network call, so load_credentials can fall back to login_client_credentials.
    let config = OAuthConfig {
        client_id: "id".to_string(),
        client_secret: "secret".to_string(),
        redirect_uri: OAuthConfig::REDIRECT_URI.to_string(),
    };
    let creds = Credentials {
        access_token: "access".to_string(),
        refresh_token: None,
        expires_at: 0,
        cloud_id: "cloud-123".to_string(),
        site_url: None,
    };

    let result = refresh(&config, &creds);

    assert!(matches!(result, Err(LoginError::Internal(_))));
}

#[test]
fn credentials_path_is_under_given_cli_dir() {
    let path = credentials_path(Path::new("/home/user/.config"), "confluence-cli");

    assert_eq!(
        path,
        PathBuf::from("/home/user/.config/confluence-cli/credentials.json")
    );
}

#[test]
fn app_config_path_is_under_given_cli_dir() {
    let path = app_config_path(Path::new("/home/user/.config"), "confluence-cli");

    assert_eq!(
        path,
        PathBuf::from("/home/user/.config/confluence-cli/app.json")
    );
}

#[test]
fn app_config_path_differs_per_cli_dir() {
    // Different crates must never resolve to the same config path.
    let jira_path = app_config_path(Path::new("/home/user/.config"), "jira-cli");
    let confluence_path = app_config_path(Path::new("/home/user/.config"), "confluence-cli");

    assert_ne!(jira_path, confluence_path);
}

#[test]
fn parses_oauth_config_from_app_json() {
    let json = r#"{"client_id": "abc", "client_secret": "shh"}"#;

    let config = OAuthConfig::from_json(json).expect("should parse");

    assert_eq!(
        config,
        OAuthConfig {
            client_id: "abc".to_string(),
            client_secret: "shh".to_string(),
            redirect_uri: OAuthConfig::REDIRECT_URI.to_string(),
        }
    );
}

#[test]
fn rejects_malformed_app_json() {
    let result = OAuthConfig::from_json("not json");

    assert!(matches!(result, Err(OAuthConfigError::InvalidJson(_))));
}

#[test]
fn rejects_app_json_missing_client_id() {
    let result = OAuthConfig::from_json(r#"{"client_secret": "shh"}"#);

    assert!(matches!(result, Err(OAuthConfigError::InvalidJson(_))));
}

#[test]
fn rejects_app_json_missing_client_secret() {
    let result = OAuthConfig::from_json(r#"{"client_id": "abc"}"#);

    assert!(matches!(result, Err(OAuthConfigError::InvalidJson(_))));
}

#[test]
fn accepts_app_json_with_extra_fields() {
    // serde ignores unknown fields — extra keys in app.json must not break loading.
    let json = r#"{"client_id": "abc", "client_secret": "shh", "extra": "ignored"}"#;

    let config = OAuthConfig::from_json(json).expect("should parse");

    assert_eq!(config.client_id, "abc");
    assert_eq!(config.client_secret, "shh");
}

#[test]
fn credentials_json_field_names_are_stable() {
    // Regression guard: if serde field names change, existing credentials.json files break.
    let creds = Credentials {
        access_token: "at".to_string(),
        refresh_token: Some("rt".to_string()),
        expires_at: 1_000,
        cloud_id: "cid".to_string(),
        site_url: Some("https://example.atlassian.net".to_string()),
    };

    let json = serde_json::to_string(&creds).unwrap();

    assert!(json.contains("\"access_token\""));
    assert!(json.contains("\"refresh_token\""));
    assert!(json.contains("\"expires_at\""));
    assert!(json.contains("\"cloud_id\""));
    assert!(json.contains("\"site_url\""));
}

#[test]
fn merge_scopes_for_cloud_id_unions_multiple_entries_with_the_same_id() {
    // Regression: Atlassian's accessible-resources endpoint returns one entry
    // PER PRODUCT when a single token's scopes span more than one product
    // (e.g. a Service Account credential granted both Jira and Confluence
    // scopes) — all sharing the same site `id`, each holding only that
    // product's scope subset, not the union. Taking just the first matching
    // entry (the original bug) silently under-reported the granted scopes.
    // Discovered live: `jira doctor`'s oauth_scopes check showed only the
    // Confluence scopes for a token that also had Jira scopes granted,
    // because the Confluence entry happened to come first in the response.
    let resources = vec![
        AccessibleResource {
            id: "site-1".to_string(),
            scopes: vec!["read:confluence-user".to_string(), "search:confluence".to_string()],
            url: None,
        },
        AccessibleResource {
            id: "site-1".to_string(),
            scopes: vec!["read:jira-work".to_string()],
            url: None,
        },
    ];

    let merged = merge_scopes_for_cloud_id(&resources, "site-1").expect("should find matching id");

    assert_eq!(
        merged,
        vec![
            "read:confluence-user".to_string(),
            "search:confluence".to_string(),
            "read:jira-work".to_string(),
        ]
    );
}

#[test]
fn merge_scopes_for_cloud_id_returns_none_when_no_entry_matches() {
    let resources = vec![AccessibleResource {
        id: "other-site".to_string(),
        scopes: vec!["read:jira-work".to_string()],
        url: None,
    }];

    assert_eq!(merge_scopes_for_cloud_id(&resources, "site-1"), None);
}

#[test]
fn merge_scopes_for_cloud_id_dedupes_overlapping_scopes_across_entries() {
    let resources = vec![
        AccessibleResource {
            id: "site-1".to_string(),
            scopes: vec!["read:jira-work".to_string()],
            url: None,
        },
        AccessibleResource {
            id: "site-1".to_string(),
            scopes: vec!["read:jira-work".to_string(), "write:jira-work".to_string()],
            url: None,
        },
    ];

    let merged = merge_scopes_for_cloud_id(&resources, "site-1").expect("should find matching id");

    assert_eq!(
        merged,
        vec!["read:jira-work".to_string(), "write:jira-work".to_string()]
    );
}

#[test]
fn merge_scopes_for_cloud_id_returns_some_empty_vec_when_entry_has_no_scopes() {
    // Present-but-empty is a different case from "not found at all" — an
    // entry that matches the cloud_id but grants zero scopes is still a
    // legitimate (if useless) resource, not a NoAccessibleResources error.
    let resources = vec![AccessibleResource {
        id: "site-1".to_string(),
        scopes: vec![],
        url: None,
    }];

    assert_eq!(merge_scopes_for_cloud_id(&resources, "site-1"), Some(vec![]));
}

// Guard: the listener only accepts requests on CALLBACK_PATH at
// CALLBACK_LISTEN_ADDR's port, so the redirect URI sent to the provider must
// point exactly there, or the login gets a 404 / WrongPath instead of the code.
#[test]
fn redirect_uri_points_at_the_callback_listener() {
    let port = crate::endpoints::CALLBACK_LISTEN_ADDR.rsplit_once(':').unwrap().1;

    assert_eq!(OAuthConfig::REDIRECT_URI, format!("http://localhost:{port}{}", crate::endpoints::CALLBACK_PATH));
}

// ── remote (two-step) login ───────────────────────────────────────────────

const REMOTE_URI: &str = "https://mercury.example.com/oauth/callback";
const NOW: u64 = 1_800_000_000;

fn local_config() -> OAuthConfig {
    OAuthConfig {
        client_id: "cid".to_string(),
        client_secret: "shh".to_string(),
        redirect_uri: OAuthConfig::REDIRECT_URI.to_string(),
    }
}

fn query(url: &str) -> std::collections::HashMap<String, String> {
    serde_urlencoded::from_str(url.split_once('?').unwrap().1).unwrap()
}

#[test]
fn pending_login_path_is_under_given_cli_dir() {
    assert_eq!(
        pending_login_path(Path::new("/cfg"), "jira-cli"),
        PathBuf::from("/cfg/jira-cli/pending-login.json")
    );
}

#[test]
fn authorization_code_body_carries_the_given_redirect_and_verifier() {
    assert_eq!(
        authorization_code_body(&local_config(), "code-1", "verifier-1", REMOTE_URI),
        serde_json::json!({
            "grant_type": "authorization_code",
            "client_id": "cid",
            "client_secret": "shh",
            "code": "code-1",
            "redirect_uri": REMOTE_URI,
            "code_verifier": "verifier-1",
        })
    );
}

#[test]
fn remote_start_saves_a_pending_login_and_builds_its_authorize_url() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("pending-login.json");

    let (url, pending) = start_remote_login(&local_config(), TEST_SCOPES, REMOTE_URI, &path, NOW).unwrap();

    let params = query(&url);
    assert!(url.starts_with("https://auth.atlassian.com/authorize?"), "got {url}");
    assert_eq!(params["redirect_uri"], REMOTE_URI);
    assert_eq!(params["state"], pending.state);
    assert_eq!(params["code_challenge"], pending.code_challenge().unwrap());
    assert_eq!(params["scope"], TEST_SCOPES);
    assert_eq!(params["audience"], "api.atlassian.com");
    assert_eq!(params["prompt"], "consent");
    assert_eq!(pending.redirect_uri.as_deref(), Some(REMOTE_URI));
    assert_eq!(pending.expires_at, NOW + 600);
    let on_disk: oauth_user_login::PendingLogin =
        serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    assert_eq!(on_disk, pending);
}

#[test]
fn remote_complete_without_a_pending_login_fails_before_any_request() {
    let dir = tempfile::tempdir().unwrap();

    let err = complete_remote_login(&local_config(), &dir.path().join("pending-login.json"), "c", "s", NOW)
        .unwrap_err();

    assert!(
        matches!(err, LoginError::PendingLogin(oauth_user_login::PendingLoginError::NotFound)),
        "got {err:?}"
    );
}

#[test]
fn remote_complete_with_a_wrong_state_keeps_the_pending_login() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("pending-login.json");
    start_remote_login(&local_config(), TEST_SCOPES, REMOTE_URI, &path, NOW).unwrap();

    let err = complete_remote_login(&local_config(), &path, "c", "stale", NOW).unwrap_err();

    assert!(
        matches!(err, LoginError::PendingLogin(oauth_user_login::PendingLoginError::StateMismatch)),
        "got {err:?}"
    );
    assert!(path.exists());
}

#[test]
fn remote_complete_after_expiry_fails_and_removes_the_pending_login() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("pending-login.json");
    let (_, pending) = start_remote_login(&local_config(), TEST_SCOPES, REMOTE_URI, &path, NOW).unwrap();

    let err = complete_remote_login(&local_config(), &path, "c", &pending.state, NOW + 600).unwrap_err();

    assert!(
        matches!(err, LoginError::PendingLogin(oauth_user_login::PendingLoginError::Expired)),
        "got {err:?}"
    );
    assert!(!path.exists());
}

// ── remote login wiring, against a local stand-in for the two endpoints ───

/// Serves `responses` in order (one per request) and returns the raw requests.
fn mock_server(responses: Vec<(&'static str, String)>) -> (String, std::thread::JoinHandle<Vec<String>>) {
    use std::io::{BufRead, BufReader, Read, Write};
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let handle = std::thread::spawn(move || {
        responses
            .into_iter()
            .map(|(status, body)| {
                let (mut stream, _) = listener.accept().unwrap();
                let mut reader = BufReader::new(stream.try_clone().unwrap());
                let mut request = String::new();
                let mut content_length = 0;
                loop {
                    let mut line = String::new();
                    reader.read_line(&mut line).unwrap();
                    if let Some(v) = line.to_ascii_lowercase().strip_prefix("content-length:") {
                        content_length = v.trim().parse().unwrap();
                    }
                    request.push_str(&line);
                    if line == "\r\n" {
                        break;
                    }
                }
                let mut body_in = vec![0; content_length];
                reader.read_exact(&mut body_in).unwrap();
                request.push_str(&String::from_utf8(body_in).unwrap());
                let response = format!(
                    "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                );
                stream.write_all(response.as_bytes()).unwrap();
                request
            })
            .collect()
    });
    (url, handle)
}

const TOKEN_OK: &str = r#"{"access_token":"at","refresh_token":"rt","expires_in":3600}"#;
const RESOURCES_OK: &str = r#"[{"id":"cloud-1","url":"https://site.example.net","scopes":["read:jira-work"]}]"#;

#[test]
fn remote_complete_sends_the_stored_verifier_and_redirect_then_resolves_the_site() {
    let (url, server) = mock_server(vec![("200 OK", TOKEN_OK.to_string()), ("200 OK", RESOURCES_OK.to_string())]);
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("pending-login.json");
    let (_, pending) = start_remote_login(&local_config(), TEST_SCOPES, REMOTE_URI, &path, NOW).unwrap();

    let creds = complete_remote_login_at(
        &local_config(), &path, "code-9", &pending.state, NOW, &format!("{url}/token"), &format!("{url}/resources"),
    )
    .unwrap();

    let requests = server.join().unwrap();
    let token_body: serde_json::Value =
        serde_json::from_str(requests[0].rsplit("\r\n\r\n").next().unwrap()).unwrap();
    assert!(requests[0].starts_with("POST /token "), "got {}", requests[0]);
    assert_eq!(
        token_body,
        authorization_code_body(&local_config(), "code-9", pending.code_verifier.as_deref().unwrap(), REMOTE_URI)
    );
    assert!(requests[1].starts_with("GET /resources "), "got {}", requests[1]);
    assert_eq!(creds.cloud_id, "cloud-1");
    assert_eq!(creds.site_url.as_deref(), Some("https://site.example.net"));
    assert_eq!(creds.refresh_token.as_deref(), Some("rt"));
    assert!(!path.exists());
}

#[test]
fn remote_complete_with_a_refused_code_is_a_token_exchange_error_and_consumes_the_state() {
    let (url, server) = mock_server(vec![("403 Forbidden", r#"{"error":"invalid_grant"}"#.to_string())]);
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("pending-login.json");
    let (_, pending) = start_remote_login(&local_config(), TEST_SCOPES, REMOTE_URI, &path, NOW).unwrap();

    let err = complete_remote_login_at(
        &local_config(), &path, "used", &pending.state, NOW, &format!("{url}/token"), &format!("{url}/resources"),
    )
    .unwrap_err();
    server.join().unwrap();

    assert!(matches!(err, LoginError::TokenExchange(_)), "got {err:?}");
    assert!(!path.exists());
}

// Regression: a failure listing the accessible sites, after the code had been
// accepted, was typed TokenExchange, so step 2 reported "Atlassian refused the
// code".
#[test]
fn a_failure_listing_sites_after_a_good_exchange_is_not_a_token_exchange_error() {
    let (url, server) = mock_server(vec![("200 OK", TOKEN_OK.to_string()), ("500 Internal Server Error", "oops".to_string())]);
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("pending-login.json");
    let (_, pending) = start_remote_login(&local_config(), TEST_SCOPES, REMOTE_URI, &path, NOW).unwrap();

    let err = complete_remote_login_at(
        &local_config(), &path, "code-9", &pending.state, NOW, &format!("{url}/token"), &format!("{url}/resources"),
    )
    .unwrap_err();
    server.join().unwrap();

    assert!(matches!(err, LoginError::AccessibleResources(_)), "got {err:?}");
    assert!(
        err.to_string().starts_with("could not list the Atlassian sites this account can access"),
        "got {err}"
    );
}
