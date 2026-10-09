#![allow(clippy::unwrap_used, clippy::expect_used)]

use atlassian_auth::{Identity, LoginError};
use oauth_user_login::{PendingLogin, PendingLoginError, UserId};
use serde_json::json;

use super::{LoginMode, remote_login_error, remote_start_output};

fn alice() -> UserId {
    UserId::parse("alice").unwrap()
}

#[test]
fn login_mode_follows_the_flags() {
    let user = Identity::User(alice());
    assert_eq!(LoginMode::from_flags(&Identity::Service, None, None, None).unwrap(), LoginMode::ServiceAccount);
    assert_eq!(LoginMode::from_flags(&user, None, None, None).unwrap(), LoginMode::UserBrowser(alice()));
    assert_eq!(
        LoginMode::from_flags(&user, Some("https://m/cb".to_string()), None, None).unwrap(),
        LoginMode::RemoteStart { id: alice(), redirect_uri: "https://m/cb".to_string() }
    );
    assert_eq!(
        LoginMode::from_flags(&user, None, Some("c".to_string()), Some("s".to_string())).unwrap(),
        LoginMode::RemoteComplete { id: alice(), code: "c".to_string(), state: "s".to_string() }
    );
}

#[test]
fn remote_login_without_user_is_rejected_with_the_corrected_command() {
    // Regression guard for issue #164: --user became global, and clap cannot
    // enforce `requires = "user"` when --user is written before the
    // subcommand, so the check moved here.
    let start = LoginMode::from_flags(&Identity::Service, Some("https://m/cb".to_string()), None, None)
        .unwrap_err()
        .to_string();
    let complete = LoginMode::from_flags(&Identity::Service, None, Some("c".to_string()), Some("s".to_string()))
        .unwrap_err()
        .to_string();

    let expected = "a remote login (--remote, --code, --state) logs in a person and needs --user <USER_ID>. \
        Retry with: jira auth login --user <USER_ID> --remote --redirect-uri <redirect-uri>, \
        then jira auth login --user <USER_ID> --code <CODE> --state <STATE>";
    assert_eq!(start, expected);
    assert_eq!(complete, expected);
}

#[test]
fn each_login_mode_saves_to_its_own_identity() {
    // A login must only ever write the credentials of the identity it names.
    let user = Identity::User(alice());
    assert_eq!(LoginMode::ServiceAccount.identity(), Identity::Service);
    assert_eq!(LoginMode::UserBrowser(alice()).identity(), user);
    assert_eq!(LoginMode::RemoteStart { id: alice(), redirect_uri: "u".to_string() }.identity(), user);
    assert_eq!(
        LoginMode::RemoteComplete { id: alice(), code: "c".to_string(), state: "s".to_string() }.identity(),
        user
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

const RESTART: &str = "Start a new remote login with: jira auth login --user alice --remote --redirect-uri <redirect-uri>";

#[test]
fn pending_login_errors_say_how_to_restart() {
    for (error, fragment) in [
        (PendingLoginError::NotFound, "there is no pending remote login"),
        (PendingLoginError::Expired, "expired"),
        (PendingLoginError::StateMismatch, "Use the code and state of the latest link"),
    ] {
        let err = remote_login_error(LoginError::PendingLogin(error), &alice()).to_string();

        assert!(err.starts_with("remote login failed: "), "got {err}");
        assert!(err.contains(fragment), "got {err}");
        assert!(err.ends_with(RESTART), "got {err}");
    }
}

#[test]
fn a_refused_code_explains_codes_are_single_use_whatever_the_status() {
    // Issue #194: a 400/401/403 answer is now TokenRejected; the message stays.
    let err = remote_login_error(LoginError::TokenRejected("403 Forbidden: invalid_grant".to_string()), &alice());

    assert_eq!(
        err.to_string(),
        format!(
            "remote login failed: Atlassian refused the code (403 Forbidden: invalid_grant). A code is \
            valid once and only for a short time, and the pending login is now used up. {RESTART}"
        )
    );
}

#[test]
fn a_refused_code_explains_codes_are_single_use() {
    let err = remote_login_error(LoginError::TokenExchange("403 Forbidden: invalid_grant".to_string()), &alice());

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
        let err = remote_login_error(error, &alice()).to_string();

        assert!(err.starts_with("remote login failed: "), "got {err}");
        assert!(!err.contains("refused the code"), "got {err}");
        assert!(err.ends_with(RESTART), "got {err}");
    }
}

// ── issue #240: whoami is the agent's login probe ─────────────────────────

#[test]
fn whoami_reports_a_refused_renewal_after_a_401_as_is_with_exit_3() {
    // Regression guard: whoami flattened every client error into
    // ApiRequestFailed (exit 1), hiding a person's revoked login.
    let renewal = crate::client::ClientError::Renewal(Box::new(crate::error::CliError::UserLoginExpired {
        reason: "400: invalid_grant".to_string(),
        id: "alice".to_string(),
    }));

    let err = super::whoami_error(renewal);

    assert!(matches!(&err, crate::error::CliError::UserLoginExpired { id, .. } if id == "alice"), "got {err:?}");
    assert_eq!(err.exit_code(), 3);
}

#[test]
fn whoami_keeps_its_message_for_other_client_errors() {
    let err = super::whoami_error(crate::client::ClientError::Status { status: 401, body: "{}".to_string() });

    assert!(matches!(&err, crate::error::CliError::ApiRequestFailed { reason } if reason == "Jira returned status 401: {}"), "got {err:?}");
    assert_eq!(err.exit_code(), 1);
}
