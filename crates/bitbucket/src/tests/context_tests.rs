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

use crate::auth::{AppConfig, Identity, LoginError, OAuthConfig, OAuthConfigError};

use super::{app_config_error, login_command, login_error_to_cli, oauth_section};

fn app(service: bool, user: bool) -> AppConfig {
    let config = || OAuthConfig { client_id: "k".to_string(), client_secret: "s".to_string() };
    AppConfig { service: service.then(config), user: user.then(config) }
}

#[test]
fn login_command_names_the_identity() {
    assert_eq!(login_command(Identity::Service), "bitbucket auth login");
    assert_eq!(login_command(Identity::User), "bitbucket auth login --user");
}

#[test]
fn oauth_section_returns_the_section_of_the_requested_identity() {
    let path = Path::new("/cfg/bitbucket-cli/app.json");

    assert!(oauth_section(app(true, false), Identity::Service, path).is_ok());
    assert!(oauth_section(app(false, true), Identity::User, path).is_ok());
}

#[test]
fn a_missing_service_section_says_to_run_init_for_the_app() {
    let err = oauth_section(app(false, true), Identity::Service, Path::new("/cfg/bitbucket-cli/app.json"))
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
    let err = oauth_section(app(true, false), Identity::User, Path::new("/cfg/bitbucket-cli/app.json"))
        .unwrap_err()
        .to_string();

    assert_eq!(
        err,
        "app.json at /cfg/bitbucket-cli/app.json has no \"user\" section (the OAuth consumer used with \
        --user to act as a human; it may be the same consumer). \
        Run: bitbucket init --user --client-id <KEY> --client-secret <SECRET>"
    );
}

#[test]
fn a_legacy_app_config_names_both_init_commands() {
    let err = app_config_error(OAuthConfigError::LegacyFormat, Path::new("/cfg/bitbucket-cli/app.json")).to_string();

    assert_eq!(
        err,
        "app.json at /cfg/bitbucket-cli/app.json uses the old single-identity format (client_id at top level). \
        Recreate it: bitbucket init --client-id <KEY> --client-secret <SECRET> for the app identity, \
        and bitbucket init --user --client-id <KEY> --client-secret <SECRET> for the human used with --user \
        (the same consumer can serve both)"
    );
}

#[test]
fn a_missing_app_config_names_both_init_commands() {
    let path = Path::new("/cfg/bitbucket-cli/app.json");
    let err = app_config_error(OAuthConfigError::NotFound(path.to_path_buf()), path).to_string();

    assert!(err.starts_with("app credentials file not found at /cfg/bitbucket-cli/app.json."), "{err}");
    assert!(err.contains("bitbucket init --client-id <KEY> --client-secret <SECRET>"), "{err}");
    assert!(err.contains("bitbucket init --user --client-id <KEY> --client-secret <SECRET>"), "{err}");
}

#[test]
fn missing_app_credentials_suggest_login_or_the_user_flag() {
    let err = login_error_to_cli(LoginError::Io(std::io::ErrorKind::NotFound.into()), Identity::Service).to_string();

    assert_eq!(
        err,
        "not logged in as the OAuth app. Run: bitbucket auth login. \
        To act as the human logged in with bitbucket auth login --user, pass --user instead"
    );
}

#[test]
fn missing_user_credentials_say_a_human_must_log_in() {
    let err = login_error_to_cli(LoginError::Io(std::io::ErrorKind::NotFound.into()), Identity::User).to_string();

    assert_eq!(
        err,
        "not logged in as a human. Run: bitbucket auth login --user (a person must approve the login in a browser)"
    );
}

#[test]
fn a_failed_renewal_names_the_login_of_the_same_identity() {
    let user = login_error_to_cli(LoginError::TokenExchange("invalid_grant".to_string()), Identity::User).to_string();
    let app = login_error_to_cli(LoginError::TokenExchange("invalid_client".to_string()), Identity::Service).to_string();

    assert_eq!(
        user,
        "failed to refresh the human's token: invalid_grant. The refresh token may have expired \
        (unused for 3 months) or been revoked. Run: bitbucket auth login --user"
    );
    assert_eq!(
        app,
        "failed to renew the OAuth app's token: invalid_client. Check that the consumer Key/Secret in \
        app.json's \"service\" section are still valid, then run: bitbucket auth login"
    );
}

#[test]
fn credentials_of_the_wrong_identity_mean_that_identity_is_not_logged_in() {
    let err = login_error_to_cli(LoginError::WrongIdentity("x"), Identity::User).to_string();

    assert!(err.starts_with("not logged in as a human. Run: bitbucket auth login --user"), "{err}");
}
