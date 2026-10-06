#![allow(clippy::unwrap_used, clippy::expect_used)]

use serde_json::json;

use atlassian_auth::Identity;

use super::{check_identities, check_pending_login};

const NOW: u64 = 1_800_000_000;

#[test]
fn pending_login_is_none_without_a_remote_login_in_progress() {
    let dir = tempfile::tempdir().unwrap();

    assert_eq!(check_pending_login(dir.path(), NOW), json!({"status": "none"}));
}

#[test]
fn pending_login_reports_its_expiry() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("confluence-cli").join("pending-login.json");
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
    let path = dir.path().join("confluence-cli").join("pending-login.json");
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(&path, "garbage").unwrap();

    let check = check_pending_login(dir.path(), NOW);

    assert_eq!(check["status"], "error");
    assert!(
        check["message"].as_str().unwrap().ends_with(
            "Start a new remote login with: confluence auth login --user --remote --redirect-uri <redirect-uri>"
        ),
        "got {check}"
    );
}

fn touch(dir: &std::path::Path, file: &str) {
    let path = dir.join("confluence-cli").join(file);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, "{}").unwrap();
}

#[test]
fn identities_reports_the_selected_identity_and_which_credentials_exist() {
    let dir = tempfile::tempdir().unwrap();
    touch(dir.path(), "credentials-user.json");

    assert_eq!(
        check_identities(dir.path(), Identity::User),
        json!({"selected": "user", "service": "missing", "user": "present", "legacy_credentials_file": false})
    );
}

#[test]
fn identities_reports_both_identities_when_both_logged_in() {
    let dir = tempfile::tempdir().unwrap();
    touch(dir.path(), "credentials-service.json");
    touch(dir.path(), "credentials-user.json");

    assert_eq!(
        check_identities(dir.path(), Identity::Service),
        json!({"selected": "service", "service": "present", "user": "present", "legacy_credentials_file": false})
    );
}

#[test]
fn identities_flags_a_leftover_pre_164_credentials_file() {
    // credentials.json is no longer read; doctor surfaces it so it can be deleted.
    let dir = tempfile::tempdir().unwrap();
    touch(dir.path(), "credentials.json");

    let check = check_identities(dir.path(), Identity::Service);

    assert_eq!(check["service"], "missing");
    assert_eq!(check["legacy_credentials_file"], true);
}
