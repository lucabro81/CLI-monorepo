#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::path::{Path, PathBuf};

use oauth_user_login::{CallbackError, WaitError};

use super::{
    app_config_path, authorization_url, credentials_path, merge_scopes_for_cloud_id, refresh,
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
