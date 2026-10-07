#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::path::{Path, PathBuf};

use super::{Identity, UserId, legacy_credentials_files, list_users, remove_identity};

fn id(s: &str) -> UserId {
    UserId::parse(s).unwrap()
}

#[test]
fn user_id_accepts_lowercase_slugs() {
    for valid in ["a", "0", "jane.doe", "m_rossi-2", "chat:u123", "a:b:c", "x".repeat(64).as_str()] {
        assert_eq!(UserId::parse(valid).unwrap().as_str(), valid, "{valid}");
    }
}

#[test]
fn user_id_rejects_anything_that_is_not_a_plain_folder_name() {
    // The id becomes a folder name under users/: anything that could escape
    // it (`..`, `/`), hide it (leading `.`) or collide by case is refused.
    let too_long = "x".repeat(65);
    for invalid in ["", ".", "..", ".hidden", "-a", "_a", "a/b", "../a", "Jane", "a b", "a@b.c", ":a", "Chat:u1", too_long.as_str()] {
        let err = UserId::parse(invalid).unwrap_err();
        assert!(err.contains("lowercase slug"), "{invalid}: {err}");
        assert!(err.contains("examples: jane.doe, chat:u123"), "{invalid}: {err}");
    }
}

#[test]
fn identity_follows_the_user_flag() {
    assert_eq!(Identity::from_user_flag(None), Identity::Service);
    assert_eq!(Identity::from_user_flag(Some(id("alice"))), Identity::User(id("alice")));
}

#[test]
fn label_names_the_identity_and_the_person() {
    assert_eq!(Identity::Service.label(), "service");
    assert_eq!(Identity::User(id("alice")).label(), "user:alice");
}

#[test]
fn each_person_has_their_own_credentials_and_pending_login_under_users() {
    let config_dir = Path::new("/home/user/.config");

    assert_eq!(
        Identity::Service.credentials_path(config_dir, "some-cli"),
        PathBuf::from("/home/user/.config/some-cli/credentials-service.json")
    );
    assert_eq!(
        Identity::User(id("alice")).credentials_path(config_dir, "some-cli"),
        PathBuf::from("/home/user/.config/some-cli/users/alice/credentials.json")
    );
    assert_eq!(
        super::pending_login_path(config_dir, "some-cli", &id("alice")),
        PathBuf::from("/home/user/.config/some-cli/users/alice/pending-login.json")
    );
}

#[test]
fn list_users_returns_sorted_ids_with_credentials_only() {
    let dir = tempfile::tempdir().unwrap();
    let users = dir.path().join("some-cli").join("users");
    for name in ["bob", "alice"] {
        std::fs::create_dir_all(users.join(name)).unwrap();
        std::fs::write(users.join(name).join("credentials.json"), "{}").unwrap();
    }
    // Only a pending remote login: not logged in yet.
    std::fs::create_dir_all(users.join("carol")).unwrap();
    std::fs::write(users.join("carol").join("pending-login.json"), "{}").unwrap();
    // Not a valid id (hand-made folder): ignored rather than reported as a person.
    std::fs::create_dir_all(users.join("Bad Name")).unwrap();
    std::fs::write(users.join("Bad Name").join("credentials.json"), "{}").unwrap();

    assert_eq!(list_users(dir.path(), "some-cli").unwrap(), vec![id("alice"), id("bob")]);
}

#[test]
fn list_users_is_empty_when_nobody_logged_in() {
    let dir = tempfile::tempdir().unwrap();
    assert!(list_users(dir.path(), "some-cli").unwrap().is_empty());
}

#[test]
fn legacy_credentials_files_reports_the_pre_175_files_that_exist() {
    let dir = tempfile::tempdir().unwrap();
    let cli = dir.path().join("some-cli");
    std::fs::create_dir_all(&cli).unwrap();
    assert!(legacy_credentials_files(dir.path(), "some-cli").is_empty());

    std::fs::write(cli.join("credentials-user.json"), "{}").unwrap();
    assert_eq!(legacy_credentials_files(dir.path(), "some-cli"), vec!["credentials-user.json"]);

    std::fs::write(cli.join("credentials.json"), "{}").unwrap();
    assert_eq!(
        legacy_credentials_files(dir.path(), "some-cli"),
        vec!["credentials.json", "credentials-user.json"]
    );
}

#[test]
fn remove_identity_deletes_only_that_persons_folder() {
    let dir = tempfile::tempdir().unwrap();
    let alice = Identity::User(id("alice"));
    let bob = Identity::User(id("bob"));
    for identity in [&alice, &bob, &Identity::Service] {
        let path = identity.credentials_path(dir.path(), "some-cli");
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, "{}").unwrap();
    }
    std::fs::write(super::pending_login_path(dir.path(), "some-cli", &id("alice")), "{}").unwrap();

    assert!(remove_identity(dir.path(), "some-cli", &alice).unwrap());

    assert!(!dir.path().join("some-cli/users/alice").exists());
    assert!(bob.credentials_path(dir.path(), "some-cli").exists());
    assert!(Identity::Service.credentials_path(dir.path(), "some-cli").exists());
}

#[test]
fn remove_identity_deletes_the_service_file_and_nothing_else() {
    let dir = tempfile::tempdir().unwrap();
    let alice = Identity::User(id("alice"));
    for identity in [&alice, &Identity::Service] {
        let path = identity.credentials_path(dir.path(), "some-cli");
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, "{}").unwrap();
    }

    assert!(remove_identity(dir.path(), "some-cli", &Identity::Service).unwrap());

    assert!(!Identity::Service.credentials_path(dir.path(), "some-cli").exists());
    assert!(alice.credentials_path(dir.path(), "some-cli").exists());
}

#[test]
fn remove_identity_reports_false_when_there_is_nothing_to_remove() {
    let dir = tempfile::tempdir().unwrap();
    assert!(!remove_identity(dir.path(), "some-cli", &Identity::User(id("alice"))).unwrap());
    assert!(!remove_identity(dir.path(), "some-cli", &Identity::Service).unwrap());
}

#[test]
fn not_logged_in_exit_code_is_distinct_from_generic_and_usage_errors() {
    // 1 is every other failure, 2 is clap's usage error.
    assert_eq!(super::NOT_LOGGED_IN_EXIT_CODE, 3);
}

#[test]
fn token_request_rejected_only_for_a_refused_grant() {
    // 400/401/403: the token endpoint refused the grant (RFC 6749 errors;
    // Atlassian answers 403 invalid_grant), so only a new login helps.
    for status in [400, 401, 403] {
        assert!(super::token_request_rejected(status, r#"{"error":"invalid_grant"}"#), "{status}");
        // Not every provider sends a JSON body.
        assert!(super::token_request_rejected(status, "Forbidden"), "{status}");
    }
    // Transient or not about the grant: retrying may work, a login would not help.
    for status in [404, 408, 429, 500, 502, 503] {
        assert!(!super::token_request_rejected(status, r#"{"error":"invalid_grant"}"#), "{status}");
    }
}

#[test]
fn a_rejected_client_is_not_a_refused_grant() {
    // Regression guard (#194 live check): Atlassian answers a wrong client id or
    // secret in app.json with 400 invalid_client; a new login would fail the
    // same way, so it must not read "log in again".
    for status in [400, 401] {
        assert!(!super::token_request_rejected(status, r#"{"error":"invalid_client","error_description":"x"}"#), "{status}");
    }
}
