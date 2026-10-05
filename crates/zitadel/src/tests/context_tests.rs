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
        r#"ZITADEL rejected the access token (401): {"message":"m"}. Run: zitadel auth login"#
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
use crate::auth::LoginError;

#[test]
fn missing_credentials_file_means_not_authenticated() {
    let err = login_error_to_cli(
        LoginError::Io(std::io::Error::from(std::io::ErrorKind::NotFound)),
        std::path::Path::new("/c/credentials.json"),
    );

    assert!(matches!(err, CliError::NotAuthenticated { .. }), "got {err:?}");
    assert!(err.to_string().ends_with("Run: zitadel auth login"));
}

#[test]
fn failed_save_after_renewal_names_the_file_not_a_relogin() {
    // Regression: this used to map to NotAuthenticated ("Run: zitadel auth login").
    let err = login_error_to_cli(
        LoginError::SaveCredentials("permission denied".to_string()),
        std::path::Path::new("/c/credentials.json"),
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
        let err = login_error_to_cli(e, std::path::Path::new("/c/credentials.json"));
        assert!(matches!(err, CliError::TokenRefreshFailed { .. }), "got {err:?}");
    }
}

#[test]
fn corrupted_credentials_mean_not_authenticated() {
    let err = login_error_to_cli(
        LoginError::InvalidCredentialsFile("eof".to_string()),
        std::path::Path::new("/c/credentials.json"),
    );

    assert!(matches!(err, CliError::NotAuthenticated { .. }), "got {err:?}");
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
    let err = CliError::UserLoginFailed { reason: "token exchange failed: 400".to_string() };

    assert_eq!(
        err.to_string(),
        "login failed: token exchange failed: 400. Check the Native application in the console: its \
        client id must match app.json (zitadel init --client-id <client-id>), its redirect URI must be \
        http://localhost:8080/callback, authentication method PKCE, refresh token enabled. Then retry: \
        zitadel auth login --user"
    );
}
