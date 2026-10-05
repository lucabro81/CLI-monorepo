#![allow(clippy::unwrap_used, clippy::expect_used)]

use oauth_user_login::{PendingLogin, PendingLoginError};
use serde_json::json;

use super::{remote_login_error, remote_start_output, LoginMode};
use crate::auth::LoginError;

#[test]
fn login_mode_follows_the_flags() {
    assert_eq!(LoginMode::from_flags(false, false, None, None), LoginMode::App);
    assert_eq!(LoginMode::from_flags(true, false, None, None), LoginMode::UserBrowser);
    assert_eq!(LoginMode::from_flags(true, true, None, None), LoginMode::RemoteStart);
    assert_eq!(
        LoginMode::from_flags(true, false, Some("c".to_string()), Some("s".to_string())),
        LoginMode::RemoteComplete { code: "c".to_string(), state: "s".to_string() }
    );
}

#[test]
fn remote_start_output_has_url_state_and_rfc3339_expiry() {
    let pending = PendingLogin::new(None, false, 1_800_000_000);

    assert_eq!(
        remote_start_output("https://bitbucket.org/site/oauth2/authorize?x=1", &pending),
        json!({
            "authorize_url": "https://bitbucket.org/site/oauth2/authorize?x=1",
            "state": pending.state,
            "expires_at": "2027-01-15T08:10:00Z",
        })
    );
}

const RESTART: &str = "Start a new remote login with: bitbucket auth login --user --remote";

#[test]
fn pending_login_errors_say_how_to_restart() {
    for (error, fragment) in [
        (PendingLoginError::NotFound, "there is no pending remote login"),
        (PendingLoginError::Expired, "expired"),
        (PendingLoginError::StateMismatch, "Use the code and state of the latest link"),
    ] {
        let err = remote_login_error(LoginError::PendingLogin(error)).to_string();

        assert!(err.starts_with("remote login failed: "), "got {err}");
        assert!(err.contains(fragment), "got {err}");
        assert!(err.ends_with(RESTART), "got {err}");
    }
}

#[test]
fn a_refused_code_explains_codes_are_single_use() {
    let err = remote_login_error(LoginError::TokenExchange("400 Bad Request: invalid_grant".to_string()));

    assert_eq!(
        err.to_string(),
        format!(
            "remote login failed: Bitbucket refused the code (400 Bad Request: invalid_grant). A code is \
            valid once and only for a short time, and the pending login is now used up. {RESTART}"
        )
    );
}
