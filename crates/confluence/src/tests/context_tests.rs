#![allow(clippy::unwrap_used, clippy::expect_used)]

use serde_json::json;

use super::print_json;
use crate::error::CliError;
use cli_fields::{RenderError, Select};

#[test]
fn required_select_returns_select_error() {
    let value = json!({"summary": "x", "status": "open"});

    let err = print_json(&value, Select::Required).expect_err("should require --select");
    match err {
        CliError::Select(RenderError::SelectRequired { size, available_fields }) => {
            assert!(size > 0);
            assert_eq!(available_fields, "top-level fields: status, summary");
        }
        other => panic!("expected CliError::Select(SelectRequired), got {other:?}"),
    }
}

#[test]
fn select_all_still_succeeds() {
    let value = json!({"summary": "x", "status": "open"});

    assert!(print_json(&value, Select::All).is_ok());
}

#[test]
fn non_empty_fields_still_succeeds() {
    let value = json!({"summary": "x", "status": "open"});

    assert!(print_json(&value, Select::Fields(&["summary"])).is_ok());
}

// --- identities (issue #164) ---

use std::path::Path;

use atlassian_auth::{AppConfig, Identity, LoginError, OAuthConfig, OAuthConfigError};
use oauth_user_login::UserId;

use super::{app_config_error, login_command, login_error_to_cli, oauth_section};

fn alice() -> Identity {
    Identity::User(UserId::parse("alice").unwrap())
}

fn app(service: bool, user: bool) -> AppConfig {
    let config = || OAuthConfig {
        client_id: "id".to_string(),
        client_secret: "secret".to_string(),
        redirect_uri: OAuthConfig::REDIRECT_URI.to_string(),
    };
    AppConfig {
        service: service.then(config),
        user: user.then(config),
    }
}

#[test]
fn login_command_names_the_identity() {
    assert_eq!(login_command(&Identity::Service), "confluence auth login");
    assert_eq!(login_command(&alice()), "confluence auth login --user alice");
}

#[test]
fn oauth_section_returns_the_section_of_the_requested_identity() {
    let path = Path::new("/cfg/confluence-cli/app.json");

    assert!(oauth_section(app(true, false), &Identity::Service, path).is_ok());
    assert!(oauth_section(app(false, true), &alice(), path).is_ok());
}

#[test]
fn a_missing_service_section_says_to_run_init_for_the_service_account() {
    let err = oauth_section(app(false, true), &Identity::Service, Path::new("/cfg/confluence-cli/app.json"))
        .unwrap_err()
        .to_string();

    assert_eq!(
        err,
        "app.json at /cfg/confluence-cli/app.json has no \"service\" section (the Service Account credential \
        used without --user). Run: confluence init --client-id <ID> --client-secret <SECRET>"
    );
}

#[test]
fn a_missing_user_section_says_to_run_init_for_the_3lo_app() {
    let err = oauth_section(app(true, false), &alice(), Path::new("/cfg/confluence-cli/app.json"))
        .unwrap_err()
        .to_string();

    assert_eq!(
        err,
        "app.json at /cfg/confluence-cli/app.json has no \"user\" section (the 3LO app every person logs in \
        with). Run: confluence init --user alice --client-id <ID> --client-secret <SECRET>"
    );
}

#[test]
fn a_legacy_app_config_names_both_init_commands() {
    let err = app_config_error(OAuthConfigError::LegacyFormat, Path::new("/cfg/confluence-cli/app.json")).to_string();

    assert_eq!(
        err,
        "app.json at /cfg/confluence-cli/app.json uses the old single-identity format (client_id at top level). \
        Recreate it: confluence init --client-id <ID> --client-secret <SECRET> for the Service Account, \
        and confluence init --user <USER_ID> --client-id <ID> --client-secret <SECRET> for the 3LO app used with --user"
    );
}

#[test]
fn a_missing_app_config_names_both_init_commands() {
    let path = Path::new("/cfg/confluence-cli/app.json");
    let err = app_config_error(OAuthConfigError::NotFound(path.to_path_buf()), path).to_string();

    assert!(err.starts_with("app credentials file not found at /cfg/confluence-cli/app.json."), "{err}");
    assert!(err.contains("confluence init --client-id <ID> --client-secret <SECRET>"), "{err}");
    assert!(err.contains("confluence init --user <USER_ID> --client-id <ID> --client-secret <SECRET>"), "{err}");
}

#[test]
fn missing_service_credentials_suggest_login_or_the_user_flag() {
    let err = login_error_to_cli(LoginError::Io(std::io::ErrorKind::NotFound.into()), Path::new("/c/credentials.json"), &Identity::Service).to_string();

    assert_eq!(
        err,
        "not logged in as the service account. Run: confluence auth login. \
        To act as a person logged in with confluence auth login --user <USER_ID>, pass --user <USER_ID> instead"
    );
}

#[test]
fn missing_user_credentials_name_the_person_and_their_login() {
    let err = login_error_to_cli(LoginError::Io(std::io::ErrorKind::NotFound.into()), Path::new("/c/credentials.json"), &alice()).to_string();

    assert_eq!(
        err,
        "user alice is not logged in. Run: confluence auth login --user alice \
        (the person must approve the login in a browser)"
    );
}

#[test]
fn a_failed_renewal_names_the_login_of_the_same_identity() {
    let user = login_error_to_cli(LoginError::TokenExchange("invalid_grant".to_string()), Path::new("/c/credentials.json"), &alice());
    let service = login_error_to_cli(LoginError::TokenExchange("invalid_grant".to_string()), Path::new("/c/credentials.json"), &Identity::Service);

    assert!(user.to_string().ends_with("Run: confluence auth login --user alice"), "{user}");
    assert!(service.to_string().ends_with("Run: confluence auth login"), "{service}");
}

#[test]
fn credentials_of_the_wrong_identity_mean_that_identity_is_not_logged_in() {
    // Regression guard (issue #164 review): a human slot without a refresh
    // token must not be renewed as the app.
    let err = login_error_to_cli(LoginError::WrongIdentity("x"), Path::new("/c/credentials.json"), &alice()).to_string();

    assert!(err.starts_with("user alice is not logged in. Run: confluence auth login --user alice"), "{err}");
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
