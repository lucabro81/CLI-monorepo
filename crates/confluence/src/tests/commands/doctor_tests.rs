#![allow(clippy::unwrap_used, clippy::expect_used)]

use serde_json::json;

use atlassian_auth::Identity;
use oauth_user_login::UserId;

use super::{check_identities, check_pending_login};

const NOW: u64 = 1_800_000_000;

fn alice() -> Identity {
    Identity::User(UserId::parse("alice").unwrap())
}

fn pending_path(dir: &std::path::Path, id: &str) -> std::path::PathBuf {
    dir.join("confluence-cli/users").join(id).join("pending-login.json")
}

#[test]
fn pending_login_is_none_without_a_remote_login_in_progress() {
    let dir = tempfile::tempdir().unwrap();

    assert_eq!(check_pending_login(dir.path(), &alice(), NOW), json!({"status": "none"}));
    assert_eq!(check_pending_login(dir.path(), &Identity::Service, NOW), json!({"status": "none"}));
}

#[test]
fn pending_login_reports_the_selected_persons_expiry() {
    let dir = tempfile::tempdir().unwrap();
    oauth_user_login::PendingLogin::new(Some("https://m/cb"), true, NOW).save(&pending_path(dir.path(), "alice")).unwrap();

    assert_eq!(
        check_pending_login(dir.path(), &alice(), NOW + 1),
        json!({"status": "pending", "expires_at": "2027-01-15T08:10:00Z", "expired": false})
    );
    assert_eq!(check_pending_login(dir.path(), &alice(), NOW + 600)["expired"], true);
    // Another person's pending login is not this one's (issue #175).
    let bob = Identity::User(UserId::parse("bob").unwrap());
    assert_eq!(check_pending_login(dir.path(), &bob, NOW), json!({"status": "none"}));
}

#[test]
fn an_unreadable_pending_login_is_reported_with_the_fix() {
    let dir = tempfile::tempdir().unwrap();
    let path = pending_path(dir.path(), "alice");
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(&path, "garbage").unwrap();

    let check = check_pending_login(dir.path(), &alice(), NOW);

    assert_eq!(check["status"], "error");
    assert!(
        check["message"].as_str().unwrap().ends_with(
            "Start a new remote login with: confluence auth login --user alice --remote --redirect-uri <redirect-uri>"
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
fn identities_reports_the_selected_identity_and_who_is_logged_in() {
    let dir = tempfile::tempdir().unwrap();
    touch(dir.path(), "users/bob/credentials.json");
    touch(dir.path(), "users/alice/credentials.json");

    assert_eq!(
        check_identities(dir.path(), &alice()),
        json!({"selected": "user:alice", "service": "missing", "users": ["alice", "bob"], "legacy_credentials_files": []})
    );
}

#[test]
fn identities_reports_the_service_identity_and_no_users() {
    let dir = tempfile::tempdir().unwrap();
    touch(dir.path(), "credentials-service.json");

    assert_eq!(
        check_identities(dir.path(), &Identity::Service),
        json!({"selected": "service", "service": "present", "users": [], "legacy_credentials_files": []})
    );
}

#[test]
fn identities_flags_leftover_credentials_files_of_earlier_layouts() {
    // Neither is read any more; doctor surfaces them so they can be deleted.
    let dir = tempfile::tempdir().unwrap();
    touch(dir.path(), "credentials.json");
    touch(dir.path(), "credentials-user.json");

    let check = check_identities(dir.path(), &Identity::Service);

    assert_eq!(check["users"], json!([]));
    assert_eq!(check["legacy_credentials_files"], json!(["credentials.json", "credentials-user.json"]));
}
