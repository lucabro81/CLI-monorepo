#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::path::Path;

use super::*;
use oauth_user_login::UserId;

fn section(client_id: &str, client_secret: &str) -> OAuthConfig {
    OAuthConfig {
        client_id: client_id.to_string(),
        client_secret: client_secret.to_string(),
    }
}

#[test]
fn parses_app_config_with_both_sections() {
    let json = r#"{
        "service": {"client_id": "app-key", "client_secret": "app-secret"},
        "user": {"client_id": "usr-key", "client_secret": "usr-secret"}
    }"#;

    let config = AppConfig::from_json(json).expect("should parse");

    assert_eq!(config.service.as_ref(), Some(&section("app-key", "app-secret")));
    assert_eq!(config.user.as_ref(), Some(&section("usr-key", "usr-secret")));
}

#[test]
fn parses_app_config_with_one_section() {
    let config = AppConfig::from_json(r#"{"user": {"client_id": "k", "client_secret": "s"}}"#)
        .expect("should parse");

    assert_eq!(config.service.as_ref(), None);
    assert_eq!(config.user.as_ref(), Some(&section("k", "s")));
}

#[test]
fn parses_app_config_with_no_sections() {
    assert_eq!(AppConfig::from_json("{}").expect("should parse"), AppConfig { service: None, user: None });
}

#[test]
fn rejects_the_legacy_flat_app_config() {
    // Before issue #164 app.json held one client_id/client_secret pair at top
    // level; it must fail loudly instead of being read as "no sections".
    let result = AppConfig::from_json(r#"{"client_id": "abc", "client_secret": "def"}"#);

    assert!(matches!(result, Err(OAuthConfigError::LegacyFormat)));
}

#[test]
fn rejects_an_app_config_section_missing_a_field() {
    assert!(matches!(
        AppConfig::from_json(r#"{"service": {"client_id": "k"}}"#),
        Err(OAuthConfigError::InvalidJson(_))
    ));
    assert!(matches!(
        AppConfig::from_json(r#"{"user": {"client_secret": "s"}}"#),
        Err(OAuthConfigError::InvalidJson(_))
    ));
}

#[test]
fn rejects_invalid_app_config_json() {
    assert!(matches!(AppConfig::from_json("not json"), Err(OAuthConfigError::InvalidJson(_))));
}

#[test]
fn load_returns_not_found_for_missing_file() {
    let result = AppConfig::load(Path::new("/nonexistent/app.json"));

    assert!(matches!(result, Err(OAuthConfigError::NotFound(_))));
}

#[test]
fn app_config_round_trips_through_json() {
    // init rewrites app.json one section at a time.
    let config = AppConfig { service: None, user: Some(section("k", "s")) };

    let json = config.to_json().expect("should serialize");

    assert_eq!(AppConfig::from_json(&json).expect("should parse"), config);
    assert!(!json.contains("\"service\""), "an absent section is omitted: {json}");
}

#[test]
fn app_config_path_is_under_bitbucket_cli_dir() {
    let path = app_config_path(Path::new("/home/user/.config"));

    assert_eq!(path, Path::new("/home/user/.config/bitbucket-cli/app.json"));
}

#[test]
fn each_identity_has_its_own_credentials_file_under_bitbucket_cli_dir() {
    let config_dir = Path::new("/home/user/.config");

    assert_eq!(
        credentials_path(config_dir, &Identity::Service),
        Path::new("/home/user/.config/bitbucket-cli/credentials-service.json")
    );
    assert_eq!(
        credentials_path(config_dir, &alice()),
        Path::new("/home/user/.config/bitbucket-cli/users/alice/credentials.json")
    );
}

#[test]
fn deserializes_real_bitbucket_token_response_shape() {
    // Regression test: Bitbucket's client_credentials token endpoint (issued via
    // auth.atlassian.com) returns the standard OAuth2 field name "scope"
    // (singular, RFC 6749), not "scopes" (plural). TokenResponse previously
    // required "scopes", so real responses failed to deserialize with
    // "error decoding response body".
    let json = r#"{"access_token":"tok","token_type":"Bearer","expires_in":7200,"scope":"repository:read pullrequest:write"}"#;

    let token: TokenResponse =
        serde_json::from_str(json).expect("should parse real Bitbucket response shape");

    assert_eq!(token.access_token, "tok");
    assert_eq!(token.expires_in, 7200);
    assert_eq!(token.scope, "repository:read pullrequest:write");
}

#[test]
fn parse_token_response_names_the_fields_it_got_on_an_unexpected_shape() {
    // Guards against a diagnosability gap: when the token endpoint returns
    // 200 with a body that doesn't match TokenResponse (e.g. an unexpected
    // field name, as happened with "scope" vs "scopes"), the error must say
    // which fields arrived, so the failure is self-diagnosing without a curl.
    // No access_token: still invalid now that "scope"/"scopes" is optional.
    let body = r#"{"expires_in":7200,"unexpected_field":"x"}"#;

    let err = parse_token_response(body).expect_err("should fail to parse");

    assert!(
        matches!(&err, LoginError::TokenExchange(msg) if msg.ends_with("; response fields: expires_in, unexpected_field")),
        "got {err}"
    );
}

// Regression: the raw body used to be copied into the error, so a 200 response
// carrying tokens in an unexpected shape printed them (e.g. in step 2 of a
// remote login).
#[test]
fn parse_token_response_never_puts_token_values_in_the_error() {
    let body = r#"{"access_token":"secret-at","refresh_token":"secret-rt","expires_in":"soon"}"#;

    let err = parse_token_response(body).expect_err("expires_in is not a number").to_string();

    assert!(!err.contains("secret-at") && !err.contains("secret-rt"), "got {err}");
    assert!(err.ends_with("; response fields: access_token, expires_in, refresh_token"), "got {err}");
}

#[test]
fn parse_token_response_on_a_non_json_body_reports_only_its_size() {
    let err = parse_token_response("<html>gateway</html>").expect_err("not JSON").to_string();

    assert!(err.ends_with("; response was not JSON (20 bytes)"), "got {err}");
    assert!(!err.contains("gateway"), "got {err}");
}

#[test]
fn parse_token_response_succeeds_on_valid_json() {
    let body = r#"{"access_token":"tok","expires_in":7200,"scope":"repository:read"}"#;

    let token = parse_token_response(body).expect("should parse");

    assert_eq!(token.access_token, "tok");
}

#[test]
fn credentials_round_trip_through_json() {
    let creds = Credentials {
        access_token: "token123".to_string(),
        expires_at: 1_700_000_000,
        scopes: vec!["repository:read".to_string(), "pullrequest:write".to_string()],
        refresh_token: Some("refresh123".to_string()),
    };

    let json = serde_json::to_string(&creds).expect("should serialize");
    let parsed: Credentials = serde_json::from_str(&json).expect("should deserialize");

    assert_eq!(parsed, creds);
}

#[test]
fn save_and_load_credentials_roundtrip_without_expiry() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("credentials.json");

    let creds = Credentials {
        access_token: "token123".to_string(),
        // Far in the future, so load_credentials doesn't try to renew over the network.
        expires_at: u64::MAX,
        scopes: vec!["repository:read".to_string()],
        refresh_token: None,
    };
    save_credentials(&path, &creds).expect("should save");

    let config = OAuthConfig {
        client_id: "ignored".to_string(),
        client_secret: "ignored".to_string(),
    };
    let loaded = load_credentials(&config, &path, &Identity::Service).expect("should load without renewing");

    assert_eq!(loaded, creds);
}

fn test_config() -> OAuthConfig {
    OAuthConfig {
        client_id: "abc".to_string(),
        client_secret: "def".to_string(),
    }
}

fn token(refresh_token: Option<&str>) -> TokenResponse {
    TokenResponse {
        access_token: "new-access".to_string(),
        expires_in: 3600,
        scope: "repository pullrequest:write".to_string(),
        refresh_token: refresh_token.map(str::to_string),
    }
}

// ── authorization_code (auth login --user) ─────────────────────────────────

#[test]
fn authorization_url_has_exact_bitbucket_params() {
    let url = authorization_url(&test_config(), "state123").expect("should build");

    assert_eq!(
        url,
        "https://bitbucket.org/site/oauth2/authorize?client_id=abc&response_type=code&state=state123"
    );
}

#[test]
fn authorization_url_encodes_query_values() {
    let config = OAuthConfig {
        client_id: "a b&c".to_string(),
        client_secret: "ignored".to_string(),
    };

    let url = authorization_url(&config, "s=1").expect("should build");

    assert_eq!(
        url,
        "https://bitbucket.org/site/oauth2/authorize?client_id=a+b%26c&response_type=code&state=s%3D1"
    );
}

// State and callback parsing are tested in crates/oauth-user-login; here only
// the wrapping, which must add this CLI's retry command.

#[test]
fn denied_consent_ends_with_the_retry_command() {
    let err = LoginError::Callback(oauth_user_login::WaitError::Callback(
        oauth_user_login::CallbackError::Denied {
            error: "access_denied".to_string(),
            description: Some("User denied access".to_string()),
        },
    ));

    assert_eq!(
        err.to_string(),
        "authorization denied: access_denied (User denied access). Approve the consent page to \
        log in, then retry the login: bitbucket auth login --user <USER_ID>"
    );
}

#[test]
fn state_mismatch_ends_with_the_retry_command() {
    let err = LoginError::Callback(oauth_user_login::WaitError::StateMismatch);

    assert!(err.to_string().ends_with("Login aborted: retry it: bitbucket auth login --user <USER_ID>"), "got {err}");
}

#[test]
fn a_busy_callback_port_names_the_port_and_the_retry_command() {
    let busy = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let address = busy.local_addr().unwrap().to_string();
    let err = LoginError::CallbackListener(oauth_user_login::bind_listener(&address).unwrap_err());

    assert!(err.to_string().starts_with(&format!("cannot listen for the login callback on {address}")), "got {err}");
    assert!(err.to_string().ends_with("and retry: bitbucket auth login --user <USER_ID>"), "got {err}");
}

#[test]
fn authorization_code_form_has_grant_and_code() {
    assert_eq!(
        authorization_code_form("XYZ"),
        vec![("grant_type", "authorization_code".to_string()), ("code", "XYZ".to_string())]
    );
}

// ── token responses ────────────────────────────────────────────────────────

#[test]
fn deserializes_token_response_with_plural_scopes_and_refresh_token() {
    // Bitbucket's documented authorization_code/refresh_token responses use
    // "scopes" (plural), while the observed client_credentials response uses
    // "scope". Both must be accepted.
    let body = r#"{"access_token":"tok","scopes":"repository account","expires_in":3600,"refresh_token":"ref","token_type":"bearer","state":"authorization_code"}"#;

    let token = parse_token_response(body).expect("should parse");

    assert_eq!(token.scope, "repository account");
    assert_eq!(token.refresh_token.as_deref(), Some("ref"));
}

#[test]
fn deserializes_token_response_without_any_scope_field() {
    let body = r#"{"access_token":"tok","expires_in":3600}"#;

    let token = parse_token_response(body).expect("should parse");

    assert_eq!(token.scope, "");
    assert_eq!(token.refresh_token, None);
}

#[test]
fn credentials_from_token_computes_expiry_and_splits_scopes() {
    let credentials = credentials_from_token(token(Some("new-refresh")), 1_000, None);

    assert_eq!(
        credentials,
        Credentials {
            access_token: "new-access".to_string(),
            expires_at: 4_600,
            scopes: vec!["repository".to_string(), "pullrequest:write".to_string()],
            refresh_token: Some("new-refresh".to_string()),
        }
    );
}

#[test]
fn credentials_from_token_prefers_rotated_refresh_token() {
    let credentials = credentials_from_token(token(Some("rotated")), 0, Some("old".to_string()));

    assert_eq!(credentials.refresh_token.as_deref(), Some("rotated"));
}

#[test]
fn credentials_from_token_keeps_previous_refresh_token_when_absent() {
    // Guards against a silent identity switch: if a refresh response ever
    // omitted refresh_token, dropping the old one would make the next renewal
    // fall back to client_credentials — i.e. quietly start acting as the app
    // instead of the human who ran `auth login --user`.
    let credentials = credentials_from_token(token(None), 0, Some("old".to_string()));

    assert_eq!(credentials.refresh_token.as_deref(), Some("old"));
}

#[test]
fn deserializes_legacy_credentials_without_refresh_token() {
    // credentials.json files written before `auth login --user` existed have
    // no refresh_token field and must keep loading as app credentials.
    let json = r#"{"access_token":"tok","expires_at":123,"scopes":["repository"]}"#;

    let credentials: Credentials = serde_json::from_str(json).expect("should parse legacy file");

    assert_eq!(credentials.refresh_token, None);
}

// ── renewal ────────────────────────────────────────────────────────────────

#[test]
fn renewal_form_uses_refresh_token_for_user_credentials() {
    let credentials = Credentials {
        access_token: "tok".to_string(),
        expires_at: 0,
        scopes: vec![],
        refresh_token: Some("ref".to_string()),
    };

    assert_eq!(
        renewal_form(&credentials),
        vec![("grant_type", "refresh_token".to_string()), ("refresh_token", "ref".to_string())]
    );
}

#[test]
fn renewal_form_uses_client_credentials_for_app_credentials() {
    let credentials = Credentials {
        access_token: "tok".to_string(),
        expires_at: 0,
        scopes: vec![],
        refresh_token: None,
    };

    assert_eq!(
        renewal_form(&credentials),
        vec![("grant_type", "client_credentials".to_string())]
    );
}

// ── remote (two-step) login ───────────────────────────────────────────────

const NOW: u64 = 1_800_000_000;

#[test]
fn each_persons_pending_login_is_under_bitbucket_cli_dir() {
    assert_eq!(
        pending_login_path(Path::new("/cfg"), &UserId::parse("alice").unwrap()),
        std::path::PathBuf::from("/cfg/bitbucket-cli/users/alice/pending-login.json")
    );
}

#[test]
fn remote_start_saves_state_only_and_builds_the_consent_url() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("pending-login.json");

    let (url, pending) = start_remote_login(&test_config(), &path, NOW).unwrap();

    assert_eq!(url, authorization_url(&test_config(), &pending.state).unwrap());
    assert_eq!(pending.redirect_uri, None);
    assert_eq!(pending.code_verifier, None);
    assert_eq!(pending.expires_at, NOW + 600);
    assert!(path.exists());
}

#[test]
fn remote_complete_without_a_pending_login_fails_before_any_request() {
    let dir = tempfile::tempdir().unwrap();

    let err = complete_remote_login(&test_config(), &dir.path().join("pending-login.json"), "c", "s", NOW)
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
    start_remote_login(&test_config(), &path, NOW).unwrap();

    let err = complete_remote_login(&test_config(), &path, "c", "stale", NOW).unwrap_err();

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
    let (_, pending) = start_remote_login(&test_config(), &path, NOW).unwrap();

    let err = complete_remote_login(&test_config(), &path, "c", &pending.state, NOW + 600).unwrap_err();

    assert!(
        matches!(err, LoginError::PendingLogin(oauth_user_login::PendingLoginError::Expired)),
        "got {err:?}"
    );
    assert!(!path.exists());
}

// ── remote login against a local stand-in for the token endpoint ──────────

fn token_server(status: &'static str, body: &'static str) -> (String, std::thread::JoinHandle<String>) {
    use std::io::{BufRead, BufReader, Read, Write};
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}/token", listener.local_addr().unwrap());
    let handle = std::thread::spawn(move || {
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
        let mut form = vec![0; content_length];
        reader.read_exact(&mut form).unwrap();
        request.push_str(&String::from_utf8(form).unwrap());
        let response = format!(
            "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        );
        stream.write_all(response.as_bytes()).unwrap();
        request
    });
    (url, handle)
}

#[test]
fn remote_complete_exchanges_the_code_and_consumes_the_pending_login() {
    let (url, server) = token_server(
        "200 OK",
        r#"{"access_token":"at","refresh_token":"rt","expires_in":7200,"scopes":"account"}"#,
    );
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("pending-login.json");
    let (_, pending) = start_remote_login(&test_config(), &path, NOW).unwrap();

    let creds = complete_remote_login_at(&test_config(), &path, "code-9", &pending.state, NOW, &url).unwrap();

    let request = server.join().unwrap();
    assert!(request.ends_with("grant_type=authorization_code&code=code-9"), "got {request}");
    assert_eq!(creds.refresh_token.as_deref(), Some("rt"));
    assert_eq!(creds.expires_at, NOW + 7200);
    assert!(!path.exists());
}

#[test]
fn remote_complete_with_a_refused_code_still_consumes_the_state() {
    let (url, server) = token_server("400 Bad Request", r#"{"error":"invalid_grant"}"#);
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("pending-login.json");
    let (_, pending) = start_remote_login(&test_config(), &path, NOW).unwrap();

    let err = complete_remote_login_at(&test_config(), &path, "used", &pending.state, NOW, &url).unwrap_err();
    server.join().unwrap();

    assert!(matches!(err, LoginError::TokenRejected(_)), "got {err:?}");
    assert!(!path.exists());
}

// Issue #194: a refused grant (Bitbucket answers 400 invalid_grant) needs a
// new login and is told apart from a transient failure, where retrying may work.
#[test]
fn a_refused_token_request_is_token_rejected_with_status_and_body() {
    for status in ["400 Bad Request", "401 Unauthorized", "403 Forbidden"] {
        let (url, server) = token_server(status, r#"{"error":"invalid_grant"}"#);
        let err = request_token_at(&url, &test_config(), &[("grant_type", "refresh_token".to_string())]).unwrap_err();
        server.join().unwrap();

        match err {
            LoginError::TokenRejected(detail) => {
                assert_eq!(detail, format!(r#"{status}: {{"error":"invalid_grant"}}"#));
            }
            other => panic!("{status}: expected TokenRejected, got {other:?}"),
        }
    }
}

#[test]
fn a_transient_token_request_failure_stays_a_token_exchange_error() {
    for status in ["429 Too Many Requests", "500 Internal Server Error", "503 Service Unavailable"] {
        let (url, server) = token_server(status, "busy");
        let err = request_token_at(&url, &test_config(), &[("grant_type", "refresh_token".to_string())]).unwrap_err();
        server.join().unwrap();

        assert!(matches!(&err, LoginError::TokenExchange(d) if d == &format!("{status}: busy")), "{status}: {err:?}");
    }
}

#[test]
fn a_refused_client_stays_a_token_exchange_error() {
    // Regression guard (#194 live check): a wrong consumer Key/Secret is
    // invalid_client; a new login through the same consumer would fail too.
    let (url, server) = token_server("401 Unauthorized", r#"{"error":"invalid_client"}"#);
    let err = request_token_at(&url, &test_config(), &[("grant_type", "refresh_token".to_string())]).unwrap_err();
    server.join().unwrap();

    assert!(matches!(err, LoginError::TokenExchange(_)), "got {err:?}");
}

fn unexpired(refresh_token: Option<&str>) -> Credentials {
    Credentials {
        access_token: "at".to_string(),
        expires_at: u64::MAX,
        scopes: vec![],
        refresh_token: refresh_token.map(str::to_string),
    }
}

fn alice() -> Identity {
    Identity::User(UserId::parse("alice").unwrap())
}

fn ignored_config() -> OAuthConfig {
    OAuthConfig { client_id: "ignored".to_string(), client_secret: "ignored".to_string() }
}

#[test]
fn a_user_slot_without_a_refresh_token_is_refused_before_any_renewal() {
    // Regression guard (issue #164 review): renewal picks client_credentials
    // when there is no refresh token, which would act as the app.
    let dir = tempfile::tempdir().unwrap();
    let path = credentials_path(dir.path(), &alice());
    save_credentials(&path, &unexpired(None)).unwrap();

    let err = load_credentials(&ignored_config(), &path, &alice()).unwrap_err();

    assert!(matches!(err, LoginError::WrongIdentity(_)), "got {err:?}");
}

#[test]
fn a_service_slot_holding_a_human_login_is_refused() {
    let dir = tempfile::tempdir().unwrap();
    let path = credentials_path(dir.path(), &Identity::Service);
    save_credentials(&path, &unexpired(Some("rt"))).unwrap();

    let err = load_credentials(&ignored_config(), &path, &Identity::Service).unwrap_err();

    assert!(matches!(err, LoginError::WrongIdentity(_)), "got {err:?}");
}

#[test]
fn saving_one_identity_leaves_the_other_identity_file_untouched() {
    let dir = tempfile::tempdir().unwrap();
    let service = credentials_path(dir.path(), &Identity::Service);
    save_credentials(&service, &unexpired(None)).unwrap();
    let before = std::fs::read(&service).unwrap();

    save_credentials(&credentials_path(dir.path(), &alice()), &unexpired(Some("rt"))).unwrap();

    assert_eq!(std::fs::read(&service).unwrap(), before);
    assert_eq!(
        load_credentials(&ignored_config(), &credentials_path(dir.path(), &alice()), &alice()).unwrap(),
        unexpired(Some("rt"))
    );
}

fn expired(refresh_token: Option<&str>) -> Credentials {
    Credentials { expires_at: 0, ..unexpired(refresh_token) }
}

fn renewed(access_token: &str) -> Credentials {
    Credentials { access_token: access_token.to_string(), ..unexpired(Some("rt-2")) }
}

#[test]
fn expired_credentials_are_renewed_once_and_saved() {
    let dir = tempfile::tempdir().unwrap();
    let path = credentials_path(dir.path(), &alice());
    save_credentials(&path, &expired(Some("rt"))).unwrap();
    let mut calls = 0;

    let loaded = load_credentials_with(&path, &alice(), |old| {
        calls += 1;
        assert_eq!(old, &expired(Some("rt")));
        Ok(renewed("fresh"))
    })
    .unwrap();

    assert_eq!(calls, 1);
    assert_eq!(loaded, renewed("fresh"));
    assert_eq!(load_credentials(&ignored_config(), &path, &alice()).unwrap(), renewed("fresh"));
}

#[test]
fn a_renewal_done_by_another_process_while_waiting_for_the_lock_is_reused() {
    // Issue #175: Bitbucket refresh tokens rotate. Two calls renewing the same
    // person at once would each spend the same refresh token; the second must
    // wait for the lock, re-read the file and keep the first one's result.
    let dir = tempfile::tempdir().unwrap();
    let path = credentials_path(dir.path(), &alice());
    save_credentials(&path, &expired(Some("rt"))).unwrap();

    let other_process = oauth_user_login::lock_exclusive(&path).unwrap();
    let waiting = {
        let path = path.clone();
        std::thread::spawn(move || load_credentials_with(&path, &alice(), |_| panic!("must not renew: already renewed")))
    };
    std::thread::sleep(std::time::Duration::from_millis(100));
    save_credentials(&path, &renewed("by-the-other-process")).unwrap();
    drop(other_process);

    assert_eq!(waiting.join().unwrap().unwrap(), renewed("by-the-other-process"));
}

#[test]
fn a_failed_renewal_leaves_the_stored_credentials_untouched() {
    let dir = tempfile::tempdir().unwrap();
    let path = credentials_path(dir.path(), &alice());
    save_credentials(&path, &expired(Some("rt"))).unwrap();

    let err = load_credentials_with(&path, &alice(), |_| Err(LoginError::TokenExchange("invalid_grant".into())))
        .unwrap_err();

    assert!(matches!(err, LoginError::TokenExchange(_)), "got {err:?}");
    let on_disk: Credentials = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    assert_eq!(on_disk, expired(Some("rt")));
}

#[cfg(unix)]
#[test]
fn saved_credentials_are_readable_only_by_the_owner() {
    // Regression for #165: credentials were written with the umask's 0644.
    use std::os::unix::fs::PermissionsExt;
    let dir = tempfile::tempdir().unwrap();
    let path = credentials_path(dir.path(), &alice());
    save_credentials(&path, &unexpired(Some("rt"))).unwrap();

    assert_eq!(std::fs::metadata(&path).unwrap().permissions().mode() & 0o777, 0o600);
}

#[test]
fn a_renewal_that_cannot_be_saved_is_a_save_failure_not_a_missing_login() {
    // Regression guard (#175 review): a failed write after a renewal used to be
    // an Io error, which callers report as "not logged in" — logging in again
    // would not fix an unwritable folder.
    let dir = tempfile::tempdir().unwrap();
    let path = credentials_path(dir.path(), &alice());
    save_credentials(&path, &expired(Some("rt"))).unwrap();
    // Credentials are written through a `<file>.tmp` sibling renamed over the
    // target; a directory in its place makes that write fail.
    let mut tmp = path.clone().into_os_string();
    tmp.push(".tmp");
    std::fs::create_dir(&tmp).unwrap();

    let err = load_credentials_with(&path, &alice(), |_| Ok(renewed("fresh"))).unwrap_err();

    assert!(matches!(err, LoginError::SaveCredentials(_)), "got {err:?}");
}

#[test]
fn a_lock_that_cannot_be_taken_is_a_save_failure_not_a_missing_login() {
    let dir = tempfile::tempdir().unwrap();
    let path = credentials_path(dir.path(), &alice());
    save_credentials(&path, &expired(Some("rt"))).unwrap();
    let mut lock = path.clone().into_os_string();
    lock.push(".lock");
    std::fs::create_dir(&lock).unwrap();

    let err = load_credentials_with(&path, &alice(), |_| panic!("must not renew without the lock")).unwrap_err();

    assert!(matches!(err, LoginError::SaveCredentials(_)), "got {err:?}");
}
