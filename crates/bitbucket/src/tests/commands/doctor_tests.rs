#![allow(clippy::unwrap_used, clippy::expect_used)]

use super::{check_pending_login, check_permissions, identity};
use crate::auth::Credentials;

fn credentials_with_scopes(scopes: Vec<&str>) -> Credentials {
    Credentials {
        access_token: "token".to_string(),
        expires_at: u64::MAX,
        scopes: scopes.into_iter().map(str::to_string).collect(),
        refresh_token: None,
    }
}

#[test]
fn identity_is_app_for_client_credentials() {
    let credentials = credentials_with_scopes(vec!["repository"]);

    assert_eq!(identity(&credentials), "app");
}

#[test]
fn identity_is_user_for_authorization_code_credentials() {
    let credentials = Credentials {
        refresh_token: Some("refresh".to_string()),
        ..credentials_with_scopes(vec!["repository"])
    };

    assert_eq!(identity(&credentials), "user");
}

#[test]
fn reports_granted_scopes_as_ok() {
    let credentials = credentials_with_scopes(vec!["repository:admin", "pullrequest:write"]);

    let report = check_permissions(&credentials);

    assert_eq!(report["status"], "ok");
    assert_eq!(report["granted_scopes"], serde_json::json!(["repository:admin", "pullrequest:write"]));
}

#[test]
fn empty_scopes_is_an_error() {
    let credentials = credentials_with_scopes(vec![]);

    let report = check_permissions(&credentials);

    assert_eq!(report["status"], "error");
    assert_eq!(report["granted_scopes"], serde_json::json!([]));
}

// ── pending_login (informational, never affects all_ok) ───────────────────

const NOW: u64 = 1_800_000_000;

#[test]
fn pending_login_is_none_without_a_remote_login_in_progress() {
    let dir = tempfile::tempdir().unwrap();

    assert_eq!(check_pending_login(dir.path(), NOW), serde_json::json!({"status": "none"}));
}

#[test]
fn pending_login_reports_its_expiry() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("bitbucket-cli").join("pending-login.json");
    oauth_user_login::PendingLogin::new(None, false, NOW).save(&path).unwrap();

    assert_eq!(
        check_pending_login(dir.path(), NOW + 1),
        serde_json::json!({"status": "pending", "expires_at": "2027-01-15T08:10:00Z", "expired": false})
    );
    assert_eq!(check_pending_login(dir.path(), NOW + 600)["expired"], true);
}

#[test]
fn an_unreadable_pending_login_is_reported_with_the_fix() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("bitbucket-cli").join("pending-login.json");
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(&path, "garbage").unwrap();

    let check = check_pending_login(dir.path(), NOW);

    assert_eq!(check["status"], "error");
    assert!(
        check["message"].as_str().unwrap().ends_with("Start a new remote login with: bitbucket auth login --user --remote"),
        "got {check}"
    );
}
