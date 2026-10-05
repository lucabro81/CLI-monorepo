#![allow(clippy::unwrap_used, clippy::expect_used)]

use oauth_user_login::{PendingLogin, PendingLoginError};
use serde_json::json;

use super::{LoginMode, remote_login_error, remote_start_output};
use crate::auth::LoginError;

// ── LoginMode ─────────────────────────────────────────────────────────────

#[test]
fn login_mode_follows_the_flags() {
    assert_eq!(LoginMode::from_flags(false, None, None, None), LoginMode::ServiceUser);
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

// ── step 1 output ─────────────────────────────────────────────────────────

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

// ── step 2 errors ─────────────────────────────────────────────────────────

const RESTART: &str = "Start a new remote login with: zitadel auth login --user --remote --redirect-uri <redirect-uri>";

#[test]
fn a_missing_pending_login_says_to_run_step_one() {
    let err = remote_login_error(LoginError::PendingLogin(PendingLoginError::NotFound));

    assert_eq!(
        err.to_string(),
        format!(
            "remote login failed: there is no pending remote login in this config folder (none was \
            started, or it was already completed). {RESTART}"
        )
    );
}

#[test]
fn an_expired_pending_login_says_to_restart() {
    let err = remote_login_error(LoginError::PendingLogin(PendingLoginError::Expired));

    assert!(err.to_string().contains("expired"), "got {err}");
    assert!(err.to_string().ends_with(RESTART), "got {err}");
}

#[test]
fn a_state_mismatch_points_to_the_latest_link() {
    let err = remote_login_error(LoginError::PendingLogin(PendingLoginError::StateMismatch));

    assert!(err.to_string().contains("Use the code and state of the latest link"), "got {err}");
    assert!(err.to_string().ends_with(RESTART), "got {err}");
}

#[test]
fn a_refused_code_explains_codes_are_single_use() {
    let err = remote_login_error(LoginError::TokenExchange("400 Bad Request: {\"error\":\"invalid_grant\"}".to_string()));

    assert_eq!(
        err.to_string(),
        format!(
            "remote login failed: ZITADEL refused the code (400 Bad Request: {{\"error\":\"invalid_grant\"}}). \
            A code is valid once and only for a short time, and the pending login is now used up. {RESTART}"
        )
    );
}

// Regression: the other step-2 failures went through UserLoginFailed, whose
// advice (redirect http://localhost:8080/callback, retry --user) is wrong for
// a remote login.
#[test]
fn every_other_step_two_failure_says_how_to_restart_without_localhost_advice() {
    let err = remote_login_error(LoginError::NoRefreshToken).to_string();

    assert!(err.starts_with("remote login failed: ZITADEL issued no refresh token"), "got {err}");
    assert!(!err.contains("localhost:8080"), "got {err}");
    assert!(err.ends_with(RESTART), "got {err}");
}
