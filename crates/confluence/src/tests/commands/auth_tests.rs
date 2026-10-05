#![allow(clippy::unwrap_used, clippy::expect_used)]

use atlassian_auth::LoginError;
use oauth_user_login::{PendingLogin, PendingLoginError};
use serde_json::json;

use super::{LoginMode, remote_login_error, remote_start_output};

#[test]
fn login_mode_follows_the_flags() {
    assert_eq!(LoginMode::from_flags(false, None, None, None), LoginMode::ServiceAccount);
    assert_eq!(LoginMode::from_flags(true, None, None, None), LoginMode::UserBrowser);
    assert_eq!(
        LoginMode::from_flags(true, Some("https://m/cb".to_string()), None, None),
        LoginMode::RemoteStart { redirect_uri: "https://m/cb".to_string() }
    );
    assert_eq!(
        LoginMode::from_flags(true, None, Some("c".to_string()), Some("s".to_string())),
        LoginMode::RemoteComplete { code: "c".to_string(), state: "s".to_string() }
    );
}

#[test]
fn remote_start_output_has_url_state_and_rfc3339_expiry_but_never_the_verifier() {
    let pending = PendingLogin::new(Some("https://m/cb"), true, 1_800_000_000);

    let output = remote_start_output("https://idp/authorize?x=1", &pending);

    assert_eq!(
        output,
        json!({
            "authorize_url": "https://idp/authorize?x=1",
            "state": pending.state,
            "expires_at": "2027-01-15T08:10:00Z",
        })
    );
    assert!(!output.to_string().contains(pending.code_verifier.as_deref().unwrap()));
}

const RESTART: &str = "Start a new remote login with: confluence auth login --user --remote --redirect-uri <redirect-uri>";

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
    let err = remote_login_error(LoginError::TokenExchange("403 Forbidden: invalid_grant".to_string()));

    assert_eq!(
        err.to_string(),
        format!(
            "remote login failed: Atlassian refused the code (403 Forbidden: invalid_grant). A code is \
            valid once and only for a short time, and the pending login is now used up. {RESTART}"
        )
    );
}

// Regression: once step 2 has consumed the pending login, every failure must
// say to restart step 1 (it used to be a bare "OAuth login failed").
#[test]
fn every_other_step_two_failure_also_says_how_to_restart() {
    for error in [LoginError::NoAccessibleResources, LoginError::AccessibleResources("500".to_string())] {
        let err = remote_login_error(error).to_string();

        assert!(err.starts_with("remote login failed: "), "got {err}");
        assert!(!err.contains("refused the code"), "got {err}");
        assert!(err.ends_with(RESTART), "got {err}");
    }
}
