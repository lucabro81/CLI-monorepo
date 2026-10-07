#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::path::PathBuf;

use atlassian_auth::Identity;
use oauth_user_login::UserId;
use serde_json::json;
use tempfile::TempDir;

use super::{AppSection, check_user_app_flag, user_app_check, write_app_config};
use crate::error::CliError;

fn alice() -> Identity {
    Identity::User(UserId::parse("alice").unwrap())
}

fn temp_config_dir() -> (TempDir, PathBuf) {
    let dir = TempDir::new().expect("tempdir");
    let path = dir.path().to_path_buf();
    (dir, path)
}

fn read_app_json(config_dir: &std::path::Path) -> serde_json::Value {
    let content = std::fs::read_to_string(config_dir.join("jira-cli").join("app.json")).expect("read");
    serde_json::from_str(&content).expect("valid JSON")
}

#[test]
fn write_app_config_writes_only_the_section_of_the_identity() {
    let (_dir, config_dir) = temp_config_dir();

    write_app_config(&config_dir, AppSection::Service, "svc-id", "svc-secret").expect("should write");

    assert_eq!(
        read_app_json(&config_dir),
        json!({"service": {"client_id": "svc-id", "client_secret": "svc-secret"}})
    );
}

#[test]
fn write_app_config_keeps_the_other_identity_section() {
    // Configuring the 3LO app must not erase the Service Account, and vice versa.
    let (_dir, config_dir) = temp_config_dir();

    write_app_config(&config_dir, AppSection::Service, "svc-id", "svc-secret").expect("first write");
    write_app_config(&config_dir, AppSection::User, "usr-id", "usr-secret").expect("second write");

    assert_eq!(
        read_app_json(&config_dir),
        json!({
            "service": {"client_id": "svc-id", "client_secret": "svc-secret"},
            "user": {"client_id": "usr-id", "client_secret": "usr-secret"}
        })
    );
}

#[test]
fn write_app_config_replaces_the_same_section() {
    let (_dir, config_dir) = temp_config_dir();

    write_app_config(&config_dir, AppSection::User, "old-id", "old-secret").expect("first write");
    write_app_config(&config_dir, AppSection::User, "new-id", "new-secret").expect("second write");

    assert_eq!(
        read_app_json(&config_dir),
        json!({"user": {"client_id": "new-id", "client_secret": "new-secret"}})
    );
}

#[test]
fn write_app_config_replaces_a_legacy_flat_file() {
    // The legacy-format error tells the caller to re-run init, so init must
    // accept that file and replace it rather than failing on it again.
    let (_dir, config_dir) = temp_config_dir();
    std::fs::create_dir_all(config_dir.join("jira-cli")).unwrap();
    std::fs::write(
        config_dir.join("jira-cli").join("app.json"),
        r#"{"client_id": "old", "client_secret": "old"}"#,
    )
    .unwrap();

    write_app_config(&config_dir, AppSection::Service, "svc-id", "svc-secret").expect("should write");

    assert_eq!(
        read_app_json(&config_dir),
        json!({"service": {"client_id": "svc-id", "client_secret": "svc-secret"}})
    );
}

#[test]
fn write_app_config_refuses_to_overwrite_an_unreadable_file() {
    let (_dir, config_dir) = temp_config_dir();
    let path = config_dir.join("jira-cli").join("app.json");
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(&path, "not json").unwrap();

    let result = write_app_config(&config_dir, AppSection::Service, "svc-id", "svc-secret");

    assert!(matches!(result, Err(CliError::AppConfigInvalid { .. })), "{result:?}");
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "not json");
}

#[test]
fn write_app_config_creates_parent_directories() {
    // config_dir may not exist yet on a fresh machine
    let (_dir, config_dir) = temp_config_dir();
    let nested = config_dir.join("does").join("not").join("exist");

    write_app_config(&nested, AppSection::User, "id", "secret").expect("should create dirs and write");

    assert!(nested.join("jira-cli").join("app.json").exists());
}

#[cfg(unix)]
#[test]
fn app_config_is_readable_only_by_the_owner() {
    // Regression for #165: app.json holds the client secrets and was written
    // with the umask's 0644.
    use std::os::unix::fs::PermissionsExt;
    let (_dir, config_dir) = temp_config_dir();

    write_app_config(&config_dir, AppSection::Service, "svc-id", "svc-secret").unwrap();

    let mode = std::fs::metadata(config_dir.join("jira-cli/app.json")).unwrap().permissions().mode();
    assert_eq!(mode & 0o777, 0o600);
}

// ── init --user-app (issue #195) ──────────────────────────────────────────

#[test]
fn section_follows_the_identity() {
    assert_eq!(AppSection::from(&Identity::Service), AppSection::Service);
    assert_eq!(AppSection::from(&alice()), AppSection::User);
}

#[test]
fn user_app_check_reports_the_user_section_once_written() {
    let (_dir, config_dir) = temp_config_dir();
    write_app_config(&config_dir, AppSection::Service, "svc-id", "svc-secret").unwrap();
    write_app_config(&config_dir, AppSection::User, "usr-id", "usr-secret").unwrap();

    let path = config_dir.join("jira-cli/app.json").display().to_string();
    assert_eq!(user_app_check(&config_dir), json!({"status": "ok", "path": path, "section": "user"}));
    // The Service Account's section is kept.
    assert_eq!(read_app_json(&config_dir)["service"]["client_id"], "svc-id");
}

#[test]
fn user_app_check_fails_without_a_user_section() {
    let (_dir, config_dir) = temp_config_dir();
    write_app_config(&config_dir, AppSection::Service, "svc-id", "svc-secret").unwrap();

    let check = user_app_check(&config_dir);

    assert_eq!(check["status"], "error");
    assert_eq!(check["section"], "user");
    assert_eq!(
        check["message"],
        "app.json has no \"user\" section. Run: jira init --user-app --client-id <ID> --client-secret <SECRET>"
    );
}

#[test]
fn user_app_check_fails_without_app_json() {
    let (_dir, config_dir) = temp_config_dir();

    let check = user_app_check(&config_dir);

    assert_eq!(check["status"], "error");
    assert!(check["message"].as_str().unwrap().starts_with("app credentials file not found at "), "{check}");
}

#[test]
fn user_app_takes_no_user_flag() {
    // --user-app logs nobody in, so naming a person is a mistake to report,
    // not to ignore (checked at runtime: clap can't see a global --user placed
    // before the subcommand).
    assert!(check_user_app_flag(&Identity::Service).is_ok());

    let err = check_user_app_flag(&alice()).unwrap_err();
    assert_eq!(
        err.to_string(),
        "init --user-app sets up the 3LO app every person logs in with and logs nobody in, so it takes no --user. \
        Run: jira init --user-app --client-id <ID> --client-secret <SECRET>, then jira auth login --user alice; \
        or jira init --user alice --client-id <ID> --client-secret <SECRET> to do both at once"
    );
}

// ── hidden secret input (issue #196) ─────────────────────────────────────

#[test]
fn on_a_terminal_the_secret_is_read_hidden() {
    // A typed secret must not echo into scrollback.
    let secret = super::read_secret_with(
        "Enter Client Secret",
        true,
        |prompt| {
            assert_eq!(prompt, "Enter Client Secret: ");
            Ok("  s3cr3t \n".to_string())
        },
        |_| panic!("the visible line reader must not be used on a terminal"),
    )
    .unwrap();

    assert_eq!(secret, "s3cr3t");
}

#[test]
fn without_a_terminal_the_secret_is_a_plain_line() {
    // Piped input (scripts) keeps working as before.
    let secret = super::read_secret_with(
        "Enter Client Secret",
        false,
        |_| panic!("no terminal: nothing to hide the input on"),
        |label| {
            assert_eq!(label, "Enter Client Secret");
            Ok("s3cr3t".to_string())
        },
    )
    .unwrap();

    assert_eq!(secret, "s3cr3t");
}

#[test]
fn a_failed_hidden_read_is_an_io_error() {
    let err = super::read_secret_with(
        "Enter Client Secret",
        true,
        |_| Err(std::io::Error::other("no tty")),
        |_| panic!("not used"),
    )
    .unwrap_err();

    assert_eq!(err.to_string(), "I/O error: could not read Enter Client Secret: no tty");
}

// ── empty prompt answers (issue #218, aligning with atlassian-admin) ──────

#[test]
fn a_typed_answer_is_kept() {
    assert_eq!(super::non_empty("abc".to_string(), "Client Secret", "--client-secret").unwrap(), "abc");
}

#[test]
fn an_empty_answer_writes_nothing() {
    // Regression guard: Enter alone at a prompt wrote an empty value into app.json.
    let err = super::non_empty(String::new(), "Client Secret", "--client-secret").unwrap_err();

    assert_eq!(
        err.to_string(),
        "Client Secret is empty: nothing was written. Run the same jira init command again and type it, or pass it with --client-secret"
    );
}
