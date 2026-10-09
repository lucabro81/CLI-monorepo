#![allow(clippy::unwrap_used, clippy::expect_used)]

use super::client_error_to_cli;
use crate::client::ClientError;
use crate::error::CliError;

fn status(status: u16) -> ClientError {
    ClientError::Status {
        status,
        body: r#"{"message":"m"}"#.to_string(),
    }
}

#[test]
fn unauthorized_points_to_auth_login() {
    assert_eq!(
        client_error_to_cli(status(401)).to_string(),
        r#"ZITADEL rejected the access token (401): {"message":"m"}. Run: zitadel auth login (zitadel auth login --user <USER_ID> if the command was run with --user <USER_ID>)"#
    );
}

#[test]
fn forbidden_keeps_the_body_and_points_to_doctor_and_roles() {
    let err = client_error_to_cli(status(403));

    assert!(matches!(&err, CliError::ApiForbidden { body } if body == r#"{"message":"m"}"#), "got {err:?}");
    assert_eq!(
        err.to_string(),
        "ZITADEL denied the operation (403): {\"message\":\"m\"}. The logged-in identity lacks the \
        administrator role this operation requires (e.g. IAM_OWNER, ORG_OWNER, ORG_USER_MANAGER). \
        Run: zitadel doctor to see its current roles, then grant the missing one in the console."
    );
}

#[test]
fn not_found_suggests_verifying_the_id() {
    assert_eq!(
        client_error_to_cli(status(404)).to_string(),
        "ZITADEL found no such resource (404): {\"message\":\"m\"}. Verify the id — find the right one with the \
        matching search/list command (e.g. zitadel user search)."
    );
}

#[test]
fn other_statuses_keep_status_and_body_and_say_what_to_do() {
    let err = client_error_to_cli(status(500));

    assert!(
        matches!(&err, CliError::ApiError { status: 500, body } if body == r#"{"message":"m"}"#),
        "got {err:?}"
    );
    assert_eq!(
        err.to_string(),
        "ZITADEL API returned status 500: {\"message\":\"m\"}. A 4xx means the request was rejected — \
        check the flags against the command's --help; a 5xx is a ZITADEL-side error — retry later."
    );
}

#[test]
fn transport_errors_map_to_request_failed() {
    let err = client_error_to_cli(ClientError::Request("connection refused".to_string()));

    assert!(
        matches!(&err, CliError::ApiRequestFailed { reason } if reason == "connection refused"),
        "got {err:?}"
    );
}

// ── login_error_to_cli (authenticated_client's credential-loading errors) ──

use super::login_error_to_cli;
use crate::auth::{Identity, LoginError, UserId};

fn alice() -> Identity {
    Identity::User(UserId::parse("alice").unwrap())
}

#[test]
fn missing_credentials_file_means_not_authenticated() {
    let err = login_error_to_cli(
        LoginError::Io(std::io::Error::from(std::io::ErrorKind::NotFound)),
        std::path::Path::new("/c/credentials-service.json"),
        &Identity::Service,
    );

    assert_eq!(
        err.to_string(),
        "not logged in as the service user (I/O error: entity not found). Run: zitadel auth login. \
        To act as a person logged in with zitadel auth login --user <USER_ID>, pass --user <USER_ID> instead"
    );
}

#[test]
fn failed_save_after_renewal_names_the_file_not_a_relogin() {
    // Regression: this used to map to NotAuthenticated ("Run: zitadel auth login").
    let err = login_error_to_cli(
        LoginError::SaveCredentials("permission denied".to_string()),
        std::path::Path::new("/c/credentials.json"),
        &Identity::Service,
    );

    assert!(
        matches!(&err, CliError::SaveCredentialsFailed { path, reason }
            if path == "/c/credentials.json" && reason == "permission denied"),
        "got {err:?}"
    );
}

#[test]
fn renewal_failures_map_to_token_refresh_failed() {
    for e in [
        LoginError::TokenExchange("400 Bad Request: invalid_grant".to_string()),
        LoginError::ServiceUserNotConfigured,
        LoginError::InvalidPrivateKey("bad pem".to_string()),
    ] {
        let err = login_error_to_cli(e, std::path::Path::new("/c/credentials.json"), &Identity::Service);
        assert!(matches!(err, CliError::TokenRenewalFailedService { .. }), "got {err:?}");
    }
}

#[test]
fn corrupted_credentials_mean_not_authenticated() {
    let err = login_error_to_cli(
        LoginError::InvalidCredentialsFile("eof".to_string()),
        std::path::Path::new("/c/credentials-user.json"),
        &alice(),
    );

    assert!(matches!(err, CliError::NotAuthenticatedUser { .. }), "got {err:?}");
}

#[test]
fn missing_user_credentials_name_the_person_and_their_login() {
    let err = login_error_to_cli(
        LoginError::Io(std::io::Error::from(std::io::ErrorKind::NotFound)),
        std::path::Path::new("/c/credentials-user.json"),
        &alice(),
    );

    assert_eq!(
        err.to_string(),
        "user alice is not logged in (I/O error: entity not found). \
        Run: zitadel auth login --user alice (the person must approve the login in a browser)"
    );
}

#[test]
fn a_failed_user_refresh_says_to_log_in_again_as_the_human() {
    let err = login_error_to_cli(
        LoginError::TokenExchange("400 Bad Request: invalid_grant".to_string()),
        std::path::Path::new("/c/credentials-user.json"),
        &alice(),
    );

    assert_eq!(
        err.to_string(),
        "failed to refresh the token of user alice: token exchange failed: 400 Bad Request: invalid_grant. \
        The refresh token may have expired or been revoked. Run: zitadel auth login --user alice"
    );
}

#[test]
fn login_failure_says_what_to_check_and_how_to_replace_the_key() {
    let err = CliError::LoginFailed {
        reason: "token exchange failed: 400 Bad Request: invalid_grant".to_string(),
    };

    assert_eq!(
        err.to_string(),
        "login failed: token exchange failed: 400 Bad Request: invalid_grant. Check that instance_url \
        in app.json is right and that the service user key still exists in the console \
        (Users > Service Users > <user> > Keys); if it was deleted or expired, create a new JSON key \
        and run: zitadel init --key-file <path-to-key.json>"
    );
}

#[test]
fn user_login_failure_points_at_the_native_app_not_the_service_user_key() {
    let err = CliError::UserLoginFailed { reason: "token exchange failed: 400".to_string(), id: "alice".to_string() };

    assert_eq!(
        err.to_string(),
        "login failed: token exchange failed: 400. Check the Native application in the console: its \
        client id must match app.json (zitadel init --client-id <client-id>), its redirect URI must be \
        http://localhost:8080/callback, authentication method PKCE, refresh token enabled. Then retry: \
        zitadel auth login --user alice"
    );
}

#[test]
fn credentials_of_the_wrong_identity_mean_that_identity_is_not_logged_in() {
    let err = login_error_to_cli(
        LoginError::WrongIdentity("x"),
        std::path::Path::new("/c/credentials-user.json"),
        &alice(),
    );

    assert!(matches!(err, CliError::NotAuthenticatedUser { .. }), "got {err:?}");
}

// ── issue #194: exit code 3 only when the identity needs a new login ──────

#[test]
fn a_refused_refresh_of_a_person_asks_for_their_login_and_exits_3() {
    let err = login_error_to_cli(
        LoginError::TokenRejected("400 Bad Request: {\"error\":\"invalid_grant\"}".to_string()),
        std::path::Path::new("/c/credentials.json"),
        &alice(),
    );

    assert_eq!(
        err.to_string(),
        "the login of user alice is no longer valid (400 Bad Request: {\"error\":\"invalid_grant\"}): \
        the refresh token expired or was revoked. Run: zitadel auth login --user alice \
        (the person must approve the login in a browser)"
    );
    assert_eq!(err.exit_code(), 3);
}

#[test]
fn a_refused_service_renewal_is_not_a_missing_login() {
    // The service user's grant is its own key in app.json: a refusal means the
    // key is wrong, and logging in again with it would not help.
    let err = login_error_to_cli(
        LoginError::TokenRejected("400 Bad Request: invalid_grant".to_string()),
        std::path::Path::new("/c/credentials.json"),
        &Identity::Service,
    );

    assert!(matches!(err, CliError::TokenRenewalFailedService { .. }), "{err:?}");
    assert_eq!(err.exit_code(), 1);
}

#[test]
fn a_transient_renewal_failure_exits_1() {
    // Network errors, 429, 5xx: retrying may work, a new login is not needed.
    let err = login_error_to_cli(
        LoginError::TokenExchange("503 Service Unavailable".to_string()),
        std::path::Path::new("/c/credentials.json"),
        &alice(),
    );

    assert!(matches!(err, CliError::TokenRefreshFailedUser { .. }), "{err:?}");
    assert_eq!(err.exit_code(), 1);
}

#[test]
fn a_missing_mismatched_or_corrupted_login_exits_3() {
    let missing = || LoginError::Io(std::io::ErrorKind::NotFound.into());
    for (error, identity) in [
        (missing(), Identity::Service),
        (missing(), alice()),
        (LoginError::WrongIdentity("x"), Identity::Service),
        (LoginError::WrongIdentity("x"), alice()),
        (LoginError::InvalidCredentialsFile("eof".to_string()), alice()),
    ] {
        let err = login_error_to_cli(error, std::path::Path::new("/c/credentials.json"), &identity);
        assert_eq!(err.exit_code(), 3, "{err}");
    }
}

#[test]
fn an_unreadable_credentials_file_is_an_io_error_not_a_missing_login() {
    // It exists but can't be read (permissions): a new login would hit the same wall.
    let err = login_error_to_cli(
        LoginError::Io(std::io::Error::new(std::io::ErrorKind::PermissionDenied, "permission denied")),
        std::path::Path::new("/c/credentials.json"),
        &alice(),
    );

    assert_eq!(err.to_string(), "I/O error: could not read /c/credentials.json: permission denied");
    assert_eq!(err.exit_code(), 1);
}

#[test]
fn every_other_error_exits_1() {
    for err in [
        CliError::ApiError { status: 401, body: "unauthorized".to_string() },
        CliError::SaveCredentialsFailed { path: "/c".to_string(), reason: "x".to_string() },
        CliError::NothingToLogOut { label: "user:alice".to_string(), login: "zitadel auth login --user alice".to_string() },
        CliError::DoctorCheckFailed,
    ] {
        assert_eq!(err.exit_code(), 1, "{err}");
    }
}

// ── issue #240: renewing after a 401 ──────────────────────────────────────

use super::rejected_token_renewer;
use crate::auth::{AppConfig, Credentials, credentials_path, save_credentials};
use crate::test_support::one_shot_server;

fn native_app(instance_url: &str) -> AppConfig {
    AppConfig { instance_url: instance_url.to_string(), service_user: None, client_id: Some("app-1".to_string()) }
}

fn person_login() -> Credentials {
    Credentials { access_token: "at-revoked".to_string(), refresh_token: Some("rt".to_string()), expires_at: u64::MAX }
}

#[test]
fn the_renewer_refreshes_a_rejected_token_and_saves_it() {
    let (url, server) =
        one_shot_server("200 OK", r#"{"access_token":"at-fresh","refresh_token":"rt-2","expires_in":3600}"#);
    let dir = tempfile::tempdir().unwrap();
    let path = credentials_path(dir.path(), &alice());
    save_credentials(&path, &person_login()).unwrap();

    let token = rejected_token_renewer(native_app(&url), path.clone(), alice())("at-revoked").unwrap();

    assert!(server.join().unwrap().contains("grant_type=refresh_token"));
    assert_eq!(token, "at-fresh");
    let saved: Credentials = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    assert_eq!((saved.access_token.as_str(), saved.refresh_token.as_deref()), ("at-fresh", Some("rt-2")));
}

#[test]
fn a_refused_refresh_after_a_401_asks_for_the_persons_login_and_exits_3() {
    // Issue #240: the person ended their ZITADEL session, so both the access
    // token and the refresh token are dead; the agent must see exit 3.
    let (url, server) = one_shot_server("400 Bad Request", r#"{"error":"invalid_grant"}"#);
    let dir = tempfile::tempdir().unwrap();
    let path = credentials_path(dir.path(), &alice());
    save_credentials(&path, &person_login()).unwrap();

    let err = rejected_token_renewer(native_app(&url), path, alice())("at-revoked").unwrap_err();

    server.join().unwrap();
    assert!(matches!(&err, CliError::UserLoginExpired { id, .. } if id == "alice"), "got {err:?}");
    assert_eq!(err.exit_code(), 3);
}

#[test]
fn a_renewal_error_reaches_the_user_unchanged() {
    let renewal = ClientError::Renewal(Box::new(CliError::UserLoginExpired {
        reason: "r".to_string(),
        id: "alice".to_string(),
    }));

    let err = client_error_to_cli(renewal);

    assert!(matches!(&err, CliError::UserLoginExpired { id, .. } if id == "alice"), "got {err:?}");
    assert_eq!(err.exit_code(), 3);
}
