#![allow(clippy::unwrap_used, clippy::expect_used)]

use serde_json::json;

use super::{print_json, split_repository};
use crate::error::CliError;
use cli_fields::{RenderError, Select};

#[test]
fn splits_workspace_and_repo_slug() {
    let (workspace, repo_slug) = split_repository("lucabrognaracode/my-repo").expect("should split");

    assert_eq!(workspace, "lucabrognaracode");
    assert_eq!(repo_slug, "my-repo");
}

#[test]
fn rejects_repository_without_slash() {
    let err = split_repository("my-repo").expect_err("should reject");

    assert!(matches!(err, CliError::InvalidRepository { value } if value == "my-repo"));
}

#[test]
fn rejects_repository_with_empty_workspace_or_slug() {
    assert!(matches!(
        split_repository("/my-repo"),
        Err(CliError::InvalidRepository { .. })
    ));
    assert!(matches!(
        split_repository("lucabrognaracode/"),
        Err(CliError::InvalidRepository { .. })
    ));
}

#[test]
fn required_select_returns_select_error() {
    let value = json!({"uuid": "x", "display_name": "y"});

    let err = print_json(&value, Select::Required).expect_err("should require --select");
    match err {
        CliError::Select(RenderError::SelectRequired { size, available_fields }) => {
            assert!(size > 0);
            assert_eq!(available_fields, "top-level fields: display_name, uuid");
        }
        other => panic!("expected CliError::Select(SelectRequired), got {other:?}"),
    }
}

#[test]
fn select_all_still_succeeds() {
    let value = json!({"uuid": "x"});

    assert!(print_json(&value, Select::All).is_ok());
}

#[test]
fn non_empty_fields_still_succeeds() {
    let value = json!({"uuid": "x"});

    assert!(print_json(&value, Select::Fields(&["uuid"])).is_ok());
}

// --- identities (issue #164) ---

use std::path::Path;

use crate::auth::{AppConfig, Identity, LoginError, OAuthConfig, OAuthConfigError, UserId};

use super::{app_config_error, login_command, login_error_to_cli, oauth_section};

fn alice() -> Identity {
    Identity::User(UserId::parse("alice").unwrap())
}

fn app(service: bool, user: bool) -> AppConfig {
    let config = || OAuthConfig { client_id: "k".to_string(), client_secret: "s".to_string() };
    AppConfig { service: service.then(config), user: user.then(config) }
}

#[test]
fn login_command_names_the_identity() {
    assert_eq!(login_command(&Identity::Service), "bitbucket auth login");
    assert_eq!(login_command(&alice()), "bitbucket auth login --user alice");
}

#[test]
fn oauth_section_returns_the_section_of_the_requested_identity() {
    let path = Path::new("/cfg/bitbucket-cli/app.json");

    assert!(oauth_section(app(true, false), &Identity::Service, path).is_ok());
    assert!(oauth_section(app(false, true), &alice(), path).is_ok());
}

#[test]
fn a_missing_service_section_says_to_run_init_for_the_app() {
    let err = oauth_section(app(false, true), &Identity::Service, Path::new("/cfg/bitbucket-cli/app.json"))
        .unwrap_err()
        .to_string();

    assert_eq!(
        err,
        "app.json at /cfg/bitbucket-cli/app.json has no \"service\" section (the OAuth consumer used \
        without --user, acting as the app). Run: bitbucket init --client-id <KEY> --client-secret <SECRET>"
    );
}

#[test]
fn a_missing_user_section_says_to_run_init_with_user() {
    let err = oauth_section(app(true, false), &alice(), Path::new("/cfg/bitbucket-cli/app.json"))
        .unwrap_err()
        .to_string();

    assert_eq!(
        err,
        "app.json at /cfg/bitbucket-cli/app.json has no \"user\" section (the OAuth consumer used with \
        --user to act as a person, every person through it; it may be the same consumer). \
        Run: bitbucket init --user alice --client-id <KEY> --client-secret <SECRET>"
    );
}

#[test]
fn a_legacy_app_config_names_both_init_commands() {
    let err = app_config_error(OAuthConfigError::LegacyFormat, Path::new("/cfg/bitbucket-cli/app.json")).to_string();

    assert_eq!(
        err,
        "app.json at /cfg/bitbucket-cli/app.json uses the old single-identity format (client_id at top level). \
        Recreate it: bitbucket init --client-id <KEY> --client-secret <SECRET> for the app identity, \
        and bitbucket init --user <USER_ID> --client-id <KEY> --client-secret <SECRET> for the people used with --user \
        (the same consumer can serve both)"
    );
}

#[test]
fn a_missing_app_config_names_both_init_commands() {
    let path = Path::new("/cfg/bitbucket-cli/app.json");
    let err = app_config_error(OAuthConfigError::NotFound(path.to_path_buf()), path).to_string();

    assert!(err.starts_with("app credentials file not found at /cfg/bitbucket-cli/app.json."), "{err}");
    assert!(err.contains("bitbucket init --client-id <KEY> --client-secret <SECRET>"), "{err}");
    assert!(err.contains("bitbucket init --user <USER_ID> --client-id <KEY> --client-secret <SECRET>"), "{err}");
}

#[test]
fn missing_app_credentials_suggest_login_or_the_user_flag() {
    let err = login_error_to_cli(LoginError::Io(std::io::ErrorKind::NotFound.into()), Path::new("/c/credentials.json"), &Identity::Service).to_string();

    assert_eq!(
        err,
        "not logged in as the OAuth app. Run: bitbucket auth login. \
        To act as a person logged in with bitbucket auth login --user <USER_ID>, pass --user <USER_ID> instead"
    );
}

#[test]
fn missing_user_credentials_name_the_person_and_their_login() {
    let err = login_error_to_cli(LoginError::Io(std::io::ErrorKind::NotFound.into()), Path::new("/c/credentials.json"), &alice()).to_string();

    assert_eq!(
        err,
        "user alice is not logged in. Run: bitbucket auth login --user alice \
        (the person must approve the login in a browser)"
    );
}

#[test]
fn a_failed_renewal_names_the_login_of_the_same_identity() {
    let user = login_error_to_cli(LoginError::TokenExchange("invalid_grant".to_string()), Path::new("/c/credentials.json"), &alice()).to_string();
    let app = login_error_to_cli(LoginError::TokenExchange("invalid_client".to_string()), Path::new("/c/credentials.json"), &Identity::Service).to_string();

    assert_eq!(
        user,
        "failed to refresh the token of user alice: invalid_grant. The refresh token may have expired \
        (unused for 3 months) or been revoked. Run: bitbucket auth login --user alice"
    );
    assert_eq!(
        app,
        "failed to renew the OAuth app's token: invalid_client. Check that the consumer Key/Secret in \
        app.json's \"service\" section are still valid, then run: bitbucket auth login"
    );
}

#[test]
fn credentials_of_the_wrong_identity_mean_that_identity_is_not_logged_in() {
    let err = login_error_to_cli(LoginError::WrongIdentity("x"), Path::new("/c/credentials.json"), &alice()).to_string();

    assert!(err.starts_with("user alice is not logged in. Run: bitbucket auth login --user alice"), "{err}");
}

#[test]
fn an_unwritable_credentials_file_names_the_file_not_a_relogin() {
    // Regression guard (#175 review): a failed save or lock after a renewal
    // used to read "not logged in", and logging in again would not fix it.
    let err = login_error_to_cli(
        LoginError::SaveCredentials("permission denied".to_string()),
        Path::new("/c/credentials.json"),
        &alice(),
    );

    assert!(
        matches!(&err, CliError::SaveCredentialsFailed { path, reason }
            if path == "/c/credentials.json" && reason == "could not write credentials file: permission denied"),
        "got {err:?}"
    );
}

// ── issue #194: exit code 3 only when the identity needs a new login ──────

#[test]
fn a_refused_refresh_of_a_person_asks_for_their_login_and_exits_3() {
    let err = login_error_to_cli(
        LoginError::TokenRejected("400 Bad Request: {\"error\":\"invalid_grant\"}".to_string()),
        Path::new("/c/credentials.json"),
        &alice(),
    );

    assert_eq!(
        err.to_string(),
        "the login of user alice is no longer valid (400 Bad Request: {\"error\":\"invalid_grant\"}): \
        the refresh token expired (unused for 3 months) or was revoked. Run: bitbucket auth login --user alice \
        (the person must approve the login in a browser)"
    );
    assert_eq!(err.exit_code(), 3);
}

#[test]
fn a_refused_service_renewal_is_not_a_missing_login() {
    // The OAuth app's grant is the consumer's own Key/Secret: a refusal means
    // app.json is wrong, and logging in again with it would not help.
    let err = login_error_to_cli(
        LoginError::TokenRejected("401 Unauthorized: invalid_client".to_string()),
        Path::new("/c/credentials.json"),
        &Identity::Service,
    );

    assert!(matches!(err, CliError::TokenRenewalFailedService { .. }), "{err:?}");
    assert_eq!(err.exit_code(), 1);
}

#[test]
fn a_transient_renewal_failure_exits_1() {
    // Network errors, 429, 5xx: retrying may work, a new login is not needed.
    let err = login_error_to_cli(LoginError::TokenExchange("503 Service Unavailable".to_string()), Path::new("/c/credentials.json"), &alice());

    assert!(matches!(err, CliError::TokenRefreshFailedUser { .. }), "{err:?}");
    assert_eq!(err.exit_code(), 1);
}

#[test]
fn a_missing_or_mismatched_login_exits_3() {
    let missing = || LoginError::Io(std::io::ErrorKind::NotFound.into());
    for (error, identity) in [
        (missing(), Identity::Service),
        (missing(), alice()),
        (LoginError::WrongIdentity("x"), Identity::Service),
        (LoginError::WrongIdentity("x"), alice()),
    ] {
        let err = login_error_to_cli(error, Path::new("/c/credentials.json"), &identity);
        assert_eq!(err.exit_code(), 3, "{err}");
    }
}

#[test]
fn an_unreadable_credentials_file_is_an_io_error_not_a_missing_login() {
    // It exists but can't be read (permissions): a new login would hit the same wall.
    let err = login_error_to_cli(
        LoginError::Io(std::io::Error::new(std::io::ErrorKind::PermissionDenied, "permission denied")),
        Path::new("/c/credentials.json"),
        &alice(),
    );

    assert_eq!(err.to_string(), "I/O error: could not read /c/credentials.json: permission denied");
    assert_eq!(err.exit_code(), 1);
}

#[test]
fn every_other_error_exits_1() {
    for err in [
        CliError::ApiRequestFailed { reason: "401 Unauthorized".to_string() },
        CliError::SaveCredentialsFailed { path: "/c".to_string(), reason: "x".to_string() },
        CliError::NothingToLogOut { label: "user:alice".to_string(), login: "bitbucket auth login --user alice".to_string() },
        CliError::UserAppMissing { path: "/c".to_string(), id: "alice".to_string() },
        CliError::DoctorCheckFailed,
    ] {
        assert_eq!(err.exit_code(), 1, "{err}");
    }
}
