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
    let err = client_error_to_cli(status(401));

    assert!(matches!(err, CliError::ApiUnauthorized { .. }), "got {err:?}");
    let msg = err.to_string();
    assert!(msg.contains("zitadel auth login"), "got {msg}");
    assert!(msg.contains(r#"{"message":"m"}"#), "body must be kept: {msg}");
}

#[test]
fn forbidden_points_to_doctor_and_roles() {
    let err = client_error_to_cli(status(403));

    assert!(matches!(err, CliError::ApiForbidden { .. }), "got {err:?}");
    let msg = err.to_string();
    assert!(msg.contains("zitadel doctor"), "got {msg}");
    assert!(msg.contains("role"), "got {msg}");
}

#[test]
fn other_statuses_keep_status_and_body() {
    let err = client_error_to_cli(status(500));

    assert!(
        matches!(&err, CliError::ApiError { status: 500, body } if body == r#"{"message":"m"}"#),
        "got {err:?}"
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

#[test]
fn not_found_suggests_verifying_the_id() {
    let err = client_error_to_cli(status(404));

    assert!(matches!(err, CliError::ApiNotFound { .. }), "got {err:?}");
    let msg = err.to_string();
    assert!(msg.contains("search"), "got {msg}");
    assert!(msg.contains(r#"{"message":"m"}"#), "body must be kept: {msg}");
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
