#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::path::Path;

use serde_json::{Value, json};

use super::{check_identities, check_pending_login, run_doctor_in, summarize_memberships};
use crate::auth::{Credentials, Identity, UserId, save_credentials};
use crate::test_support::mock_server;

const ME_MACHINE: &str = r#"{"user":{"id":"u-1","userName":"service-user","details":{"resourceOwner":"org-1"},"machine":{"name":"service-user"}}}"#;
const MEMBERSHIPS_ORG_OWNER: &str = r#"{"details":{"totalResult":"2"},"result":[
    {"userId":"u-1","roles":["ORG_OWNER"],"displayName":"Acme","orgId":"org-1"},
    {"userId":"u-1","roles":["IAM_LOGIN_CLIENT"],"displayName":"ZITADEL","iam":true}]}"#;

fn write_app_json(config_dir: &Path, instance_url: &str) {
    let dir = config_dir.join("zitadel-cli");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("app.json"),
        json!({"instance_url": instance_url}).to_string(),
    )
    .unwrap();
}

fn alice() -> Identity {
    Identity::User(UserId::parse("alice").unwrap())
}

/// Saved to the identity the credentials belong to: a refresh token means the
/// person `alice`.
fn write_valid_credentials(config_dir: &Path, refresh_token: Option<&str>) {
    let file = if refresh_token.is_some() { "users/alice/credentials.json" } else { "credentials-service.json" };
    save_credentials(
        &config_dir.join("zitadel-cli").join(file),
        &Credentials {
            access_token: "at".to_string(),
            refresh_token: refresh_token.map(str::to_string),
            expires_at: 4_000_000_000,
        },
    )
    .unwrap();
}

fn statuses(report: &Value) -> [&str; 4] {
    ["app_config", "credentials", "api", "memberships"].map(|k| report[k]["status"].as_str().unwrap())
}

#[test]
fn missing_app_config_skips_everything_else() {
    let dir = tempfile::tempdir().unwrap();

    let (report, all_ok) = run_doctor_in(dir.path(), &Identity::Service);

    assert!(!all_ok);
    assert_eq!(statuses(&report), ["error", "skipped", "skipped", "skipped"]);
    assert_eq!(
        report["app_config"]["message"],
        format!(
            "app config file not found at {}. Run: zitadel init --instance-url https://<instance>.zitadel.cloud --key-file <path-to-key.json>",
            dir.path().join("zitadel-cli/app.json").display()
        )
    );
}

#[test]
fn missing_credentials_skips_api_and_memberships() {
    let dir = tempfile::tempdir().unwrap();
    write_app_json(dir.path(), "https://acme.zitadel.cloud");

    let (report, all_ok) = run_doctor_in(dir.path(), &Identity::Service);

    assert!(!all_ok);
    assert_eq!(statuses(&report), ["ok", "error", "skipped", "skipped"]);
    assert_eq!(report["credentials"]["message"], "no stored credentials. Run: zitadel auth login");
    assert_eq!(
        report["app_config"],
        json!({
            "status": "ok",
            "path": dir.path().join("zitadel-cli/app.json").display().to_string(),
            "instance_url": "https://acme.zitadel.cloud",
            "service_user_configured": false,
            "native_app_configured": false,
        })
    );
}

#[test]
fn healthy_service_user_reports_identity_and_roles() {
    let dir = tempfile::tempdir().unwrap();
    let (url, server) = mock_server(&[("200 OK", ME_MACHINE), ("200 OK", MEMBERSHIPS_ORG_OWNER)]);
    write_app_json(dir.path(), &url);
    write_valid_credentials(dir.path(), None);

    let (report, all_ok) = run_doctor_in(dir.path(), &Identity::Service);
    let requests = server.join().unwrap();

    assert!(all_ok, "report: {report:#}");
    assert!(requests[0].starts_with("GET /auth/v1/users/me "));
    assert!(requests[1].starts_with("POST /auth/v1/memberships/me/_search "));
    assert_eq!(report["credentials"]["identity"], "service_user");
    assert_eq!(report["credentials"]["expires_at"], 4_000_000_000_u64);
    assert_eq!(
        report["api"],
        json!({"status": "ok", "user_id": "u-1", "user_name": "service-user",
               "type": "machine", "organization_id": "org-1"})
    );
    assert_eq!(
        report["memberships"],
        json!({"status": "ok", "memberships": [
            {"level": "organization", "id": "org-1", "display_name": "Acme", "roles": ["ORG_OWNER"]},
            {"level": "instance", "id": null, "display_name": "ZITADEL", "roles": ["IAM_LOGIN_CLIENT"]}
        ]})
    );
}

#[test]
fn credentials_with_refresh_token_are_reported_as_human_user() {
    let dir = tempfile::tempdir().unwrap();
    let (url, server) = mock_server(&[
        ("200 OK", r#"{"user":{"id":"h-1","userName":"luca","details":{"resourceOwner":"org-1"},"human":{}}}"#),
        ("200 OK", MEMBERSHIPS_ORG_OWNER),
    ]);
    write_app_json(dir.path(), &url);
    write_valid_credentials(dir.path(), Some("rt"));

    let (report, _) = run_doctor_in(dir.path(), &alice());
    server.join().unwrap();

    assert_eq!(report["credentials"]["identity"], "user");
    assert_eq!(report["api"]["type"], "human");
}

fn write_native_app_json(config_dir: &Path, instance_url: &str) {
    let dir = config_dir.join("zitadel-cli");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("app.json"), json!({"instance_url": instance_url, "client_id": "app-1"}).to_string())
        .unwrap();
}

const REVOKED: &str = r#"{"message":"invalid token"}"#;
const FRESH_TOKEN: &str = r#"{"access_token":"at-fresh","refresh_token":"rt-2","expires_in":3600}"#;

#[test]
fn a_revoked_token_whose_refresh_is_refused_fails_the_credentials_check() {
    // Issue #240: credentials used to be "ok" whenever expires_at was in the
    // future, although ZITADEL had revoked the token with the person's session.
    let dir = tempfile::tempdir().unwrap();
    let (url, server) =
        mock_server(&[("401 Unauthorized", REVOKED), ("400 Bad Request", r#"{"error":"invalid_grant"}"#)]);
    write_native_app_json(dir.path(), &url);
    write_valid_credentials(dir.path(), Some("rt"));

    let (report, all_ok) = run_doctor_in(dir.path(), &alice());
    let requests = server.join().unwrap();

    assert!(!all_ok);
    assert!(requests[1].contains("grant_type=refresh_token"), "got {}", requests[1]);
    assert_eq!(statuses(&report), ["ok", "error", "skipped", "skipped"]);
    let message = report["credentials"]["message"].as_str().unwrap();
    assert!(message.starts_with("the login of user alice is no longer valid"), "got {message}");
    assert!(message.contains("Run: zitadel auth login --user alice"), "got {message}");
    assert!(report["credentials"]["path"].as_str().unwrap().ends_with("users/alice/credentials.json"));
    assert_eq!(report["api"]["reason"], "credentials check failed");
}

#[test]
fn a_revoked_token_that_renews_leaves_every_check_ok() {
    let dir = tempfile::tempdir().unwrap();
    let (url, server) = mock_server(&[
        ("401 Unauthorized", REVOKED),
        ("200 OK", FRESH_TOKEN),
        ("200 OK", r#"{"user":{"id":"h-1","userName":"alice","details":{"resourceOwner":"org-1"},"human":{}}}"#),
        ("200 OK", MEMBERSHIPS_ORG_OWNER),
    ]);
    write_native_app_json(dir.path(), &url);
    write_valid_credentials(dir.path(), Some("rt"));

    let (report, all_ok) = run_doctor_in(dir.path(), &alice());
    server.join().unwrap();

    assert!(all_ok, "{report}");
    assert_eq!(statuses(&report), ["ok", "ok", "ok", "ok"]);
}

#[test]
fn a_401_with_a_freshly_renewed_token_fails_the_api_check() {
    let dir = tempfile::tempdir().unwrap();
    let (url, server) =
        mock_server(&[("401 Unauthorized", REVOKED), ("200 OK", FRESH_TOKEN), ("401 Unauthorized", REVOKED)]);
    write_native_app_json(dir.path(), &url);
    write_valid_credentials(dir.path(), Some("rt"));

    let (report, all_ok) = run_doctor_in(dir.path(), &alice());
    server.join().unwrap();

    assert!(!all_ok);
    assert_eq!(statuses(&report), ["ok", "ok", "error", "skipped"]);
    assert_eq!(
        report["api"]["message"],
        r#"ZITADEL rejected the access token (401): {"message":"invalid token"}. Run: zitadel auth login (zitadel auth login --user <USER_ID> if the command was run with --user <USER_ID>)"#
    );
}

#[test]
fn a_revoked_service_token_without_a_key_to_renew_it_fails_the_credentials_check() {
    let dir = tempfile::tempdir().unwrap();
    let (url, server) = mock_server(&[("401 Unauthorized", REVOKED)]);
    write_app_json(dir.path(), &url);
    write_valid_credentials(dir.path(), None);

    let (report, all_ok) = run_doctor_in(dir.path(), &Identity::Service);
    server.join().unwrap();

    assert!(!all_ok);
    assert_eq!(statuses(&report), ["ok", "error", "skipped", "skipped"]);
    assert!(report["credentials"]["message"].as_str().unwrap().contains("zitadel init --key-file"), "{report}");
}

#[test]
fn identity_without_memberships_is_an_error_with_a_grant_hint() {
    let dir = tempfile::tempdir().unwrap();
    let (url, server) = mock_server(&[
        ("200 OK", ME_MACHINE),
        ("200 OK", r#"{"details":{"totalResult":"0"}}"#),
    ]);
    write_app_json(dir.path(), &url);
    write_valid_credentials(dir.path(), None);

    let (report, all_ok) = run_doctor_in(dir.path(), &Identity::Service);
    server.join().unwrap();

    assert!(!all_ok);
    assert_eq!(report["memberships"]["status"], "error");
    assert_eq!(
        report["memberships"]["message"],
        "the identity has no administrator role, so every management command will be \
        denied (403). In the ZITADEL console grant it one, e.g. ORG_OWNER on an \
        organization (Organization > Administrators) or IAM_OWNER on the instance \
        (Default settings > Administrators)."
    );
}

#[test]
fn summarizes_memberships_by_level() {
    let raw = json!({"result": [
        {"roles": ["IAM_OWNER"], "displayName": "ZITADEL", "iam": true},
        {"roles": ["ORG_OWNER", "ORG_USER_MANAGER"], "displayName": "Acme", "orgId": "org-1"},
        {"roles": ["PROJECT_OWNER"], "displayName": "App", "orgId": "org-1", "projectId": "p-1"},
        {"roles": ["PROJECT_GRANT_OWNER"], "displayName": "Shared", "projectId": "p-1", "projectGrantId": "g-1"}
    ]});

    assert_eq!(
        summarize_memberships(&raw),
        vec![
            json!({"level": "instance", "id": null, "display_name": "ZITADEL", "roles": ["IAM_OWNER"]}),
            json!({"level": "organization", "id": "org-1", "display_name": "Acme", "roles": ["ORG_OWNER", "ORG_USER_MANAGER"]}),
            json!({"level": "project", "id": "p-1", "display_name": "App", "roles": ["PROJECT_OWNER"]}),
            json!({"level": "project_grant", "id": "g-1", "display_name": "Shared", "roles": ["PROJECT_GRANT_OWNER"]}),
        ]
    );
}

#[test]
fn summarize_memberships_tolerates_missing_result() {
    assert!(summarize_memberships(&json!({"details": {}})).is_empty());
}

#[test]
fn corrupted_credentials_fail_the_credentials_check() {
    let dir = tempfile::tempdir().unwrap();
    write_app_json(dir.path(), "https://acme.zitadel.cloud");
    std::fs::write(dir.path().join("zitadel-cli/credentials-service.json"), "{not json").unwrap();

    let (report, all_ok) = run_doctor_in(dir.path(), &Identity::Service);

    assert!(!all_ok);
    assert_eq!(statuses(&report), ["ok", "error", "skipped", "skipped"]);
    let message = report["credentials"]["message"].as_str().unwrap();
    assert!(message.starts_with("credentials file is corrupted ("), "got {message}");
    assert!(message.ends_with(". Run: zitadel auth login"), "got {message}");
}

#[test]
fn failing_memberships_call_is_an_error_after_a_healthy_api_check() {
    let dir = tempfile::tempdir().unwrap();
    let (url, server) = mock_server(&[
        ("200 OK", ME_MACHINE),
        ("403 Forbidden", r#"{"message":"No matching permissions found"}"#),
    ]);
    write_app_json(dir.path(), &url);
    write_valid_credentials(dir.path(), None);

    let (report, all_ok) = run_doctor_in(dir.path(), &Identity::Service);
    server.join().unwrap();

    assert!(!all_ok);
    assert_eq!(statuses(&report), ["ok", "ok", "ok", "error"]);
    assert!(
        report["memberships"]["message"]
            .as_str()
            .unwrap()
            .starts_with(r#"ZITADEL denied the operation (403): {"message":"No matching permissions found"}."#),
        "got {report:#}"
    );
}

#[test]
fn user_without_machine_or_human_object_is_reported_as_unknown_type() {
    let dir = tempfile::tempdir().unwrap();
    let (url, server) = mock_server(&[
        ("200 OK", r#"{"user":{"id":"x","userName":"x","details":{"resourceOwner":"o"}}}"#),
        ("200 OK", MEMBERSHIPS_ORG_OWNER),
    ]);
    write_app_json(dir.path(), &url);
    write_valid_credentials(dir.path(), None);

    let (report, _) = run_doctor_in(dir.path(), &Identity::Service);
    server.join().unwrap();

    assert_eq!(report["api"]["type"], "unknown");
}

// ── pending_login (informational, never affects all_ok) ───────────────────

const NOW: u64 = 1_800_000_000;

fn pending_path(dir: &Path, id: &str) -> std::path::PathBuf {
    dir.join("zitadel-cli/users").join(id).join("pending-login.json")
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
    assert_eq!(
        check_pending_login(dir.path(), &alice(), NOW + 600),
        json!({"status": "pending", "expires_at": "2027-01-15T08:10:00Z", "expired": true})
    );
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
            "Start a new remote login with: zitadel auth login --user alice --remote --redirect-uri <redirect-uri>"
        ),
        "got {check}"
    );
}

// Regression guard: pending_login is informational. A healthy report stays
// all_ok even with an expired or unreadable pending login next to it.
#[test]
fn a_pending_login_never_affects_the_overall_result() {
    for pending in ["expired", "corrupt"] {
        let dir = tempfile::tempdir().unwrap();
        let (url, server) = mock_server(&[("200 OK", ME_MACHINE), ("200 OK", MEMBERSHIPS_ORG_OWNER)]);
        write_app_json(dir.path(), &url);
        write_valid_credentials(dir.path(), Some("rt"));
        let path = pending_path(dir.path(), "alice");
        if pending == "expired" {
            oauth_user_login::PendingLogin::new(Some("https://m/cb"), true, 0).save(&path).unwrap();
        } else {
            std::fs::write(&path, "garbage").unwrap();
        }

        let (report, all_ok) = run_doctor_in(dir.path(), &alice());
        server.join().unwrap();

        assert!(all_ok, "{pending}: {report:#}");
        assert_ne!(report["pending_login"]["status"], "none", "{pending}: {report:#}");
    }
}

#[test]
fn missing_user_credentials_point_to_the_human_login() {
    let dir = tempfile::tempdir().unwrap();
    write_app_json(dir.path(), "https://acme.zitadel.cloud");
    write_valid_credentials(dir.path(), None);

    let (report, all_ok) = run_doctor_in(dir.path(), &alice());

    assert!(!all_ok);
    assert_eq!(report["credentials"]["message"], "no stored credentials. Run: zitadel auth login --user alice");
}

fn touch(dir: &Path, file: &str) {
    let path = dir.join("zitadel-cli").join(file);
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
fn identities_flags_leftover_credentials_files_of_earlier_layouts() {
    // Neither is read any more; doctor surfaces them so they can be deleted.
    let dir = tempfile::tempdir().unwrap();
    touch(dir.path(), "credentials.json");
    touch(dir.path(), "credentials-user.json");
    touch(dir.path(), "credentials-service.json");

    assert_eq!(
        check_identities(dir.path(), &Identity::Service),
        json!({
            "selected": "service",
            "service": "present",
            "users": [],
            "legacy_credentials_files": ["credentials.json", "credentials-user.json"],
        })
    );
}

#[test]
fn the_report_includes_identities() {
    let dir = tempfile::tempdir().unwrap();

    let (report, _) = run_doctor_in(dir.path(), &Identity::Service);

    assert_eq!(report["identities"]["selected"], "service");
}
