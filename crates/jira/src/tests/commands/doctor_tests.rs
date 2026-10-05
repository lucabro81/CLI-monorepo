#![allow(clippy::unwrap_used, clippy::expect_used)]

use serde_json::json;

use super::check_pending_login;

const NOW: u64 = 1_800_000_000;

#[test]
fn pending_login_is_none_without_a_remote_login_in_progress() {
    let dir = tempfile::tempdir().unwrap();

    assert_eq!(check_pending_login(dir.path(), NOW), json!({"status": "none"}));
}

#[test]
fn pending_login_reports_its_expiry() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("jira-cli").join("pending-login.json");
    oauth_user_login::PendingLogin::new(Some("https://m/cb"), true, NOW).save(&path).unwrap();

    assert_eq!(
        check_pending_login(dir.path(), NOW + 1),
        json!({"status": "pending", "expires_at": "2027-01-15T08:10:00Z", "expired": false})
    );
    assert_eq!(check_pending_login(dir.path(), NOW + 600)["expired"], true);
}

#[test]
fn an_unreadable_pending_login_is_reported_with_the_fix() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("jira-cli").join("pending-login.json");
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(&path, "garbage").unwrap();

    let check = check_pending_login(dir.path(), NOW);

    assert_eq!(check["status"], "error");
    assert!(
        check["message"].as_str().unwrap().ends_with(
            "Start a new remote login with: jira auth login --user --remote --redirect-uri <redirect-uri>"
        ),
        "got {check}"
    );
}
