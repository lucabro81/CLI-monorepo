#![allow(clippy::unwrap_used, clippy::expect_used)]

use super::{
    build_app_config, check_user_app_flag, discard_instance_credentials, instance_changed, read_key_file,
    user_app_report, write_app_config,
};
use crate::auth::{Identity, UserId};
use crate::auth::{AppConfig, ServiceUserKey};
use crate::error::CliError;

fn key(id: &str) -> ServiceUserKey {
    ServiceUserKey {
        key_id: id.to_string(),
        key: "pem".to_string(),
        user_id: "u-1".to_string(),
    }
}

fn existing() -> AppConfig {
    AppConfig {
        instance_url: "https://old.zitadel.cloud".to_string(),
        service_user: Some(key("old-key")),
        client_id: Some("old-client".to_string()),
    }
}

#[test]
fn builds_new_config_from_flags_and_normalizes_url() {
    let config =
        build_app_config(None, Some("https://acme.zitadel.cloud/"), Some(key("k")), None).unwrap();

    assert_eq!(
        config,
        AppConfig {
            instance_url: "https://acme.zitadel.cloud".to_string(),
            service_user: Some(key("k")),
            client_id: None,
        }
    );
}

#[test]
fn rerun_with_only_client_id_keeps_existing_url_and_key() {
    let config = build_app_config(Some(existing()), None, None, Some("new-client")).unwrap();

    assert_eq!(
        config,
        AppConfig {
            instance_url: "https://old.zitadel.cloud".to_string(),
            service_user: Some(key("old-key")),
            client_id: Some("new-client".to_string()),
        }
    );
}

#[test]
fn flags_override_existing_values() {
    let config = build_app_config(
        Some(existing()),
        Some("https://new.zitadel.cloud"),
        Some(key("new-key")),
        None,
    )
    .unwrap();

    assert_eq!(config.instance_url, "https://new.zitadel.cloud");
    assert_eq!(config.service_user, Some(key("new-key")));
    assert_eq!(config.client_id.as_deref(), Some("old-client"));
}

#[test]
fn missing_instance_url_without_existing_config_is_an_error() {
    let err = build_app_config(None, None, Some(key("k")), None).unwrap_err();

    assert!(matches!(err, CliError::InstanceUrlRequired), "got {err:?}");
    assert!(err.to_string().contains("--instance-url"));
}

#[test]
fn invalid_instance_url_flag_is_an_error() {
    let err = build_app_config(None, Some("acme.zitadel.cloud"), None, None).unwrap_err();

    assert!(
        matches!(&err, CliError::InvalidInstanceUrl { value } if value == "acme.zitadel.cloud"),
        "got {err:?}"
    );
}

#[test]
fn written_app_config_round_trips_and_omits_absent_fields() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("zitadel-cli").join("app.json");
    let config = AppConfig {
        instance_url: "https://acme.zitadel.cloud".to_string(),
        service_user: Some(key("k")),
        client_id: None,
    };

    write_app_config(&path, &config).unwrap();

    let raw: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    assert!(raw.get("client_id").is_none(), "got {raw}");
    assert_eq!(AppConfig::load(&path).unwrap(), config);
}

#[cfg(unix)]
#[test]
fn written_app_config_is_readable_only_by_owner() {
    // app.json holds the service user's private key.
    use std::os::unix::fs::PermissionsExt;
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("app.json");

    write_app_config(&path, &existing()).unwrap();

    let mode = std::fs::metadata(&path).unwrap().permissions().mode() & 0o777;
    assert_eq!(mode, 0o600);
}

#[cfg(unix)]
#[test]
fn rewriting_an_existing_world_readable_app_config_tightens_permissions() {
    use std::os::unix::fs::PermissionsExt;
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("app.json");
    std::fs::write(&path, "{}").unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).unwrap();

    write_app_config(&path, &existing()).unwrap();

    let mode = std::fs::metadata(&path).unwrap().permissions().mode() & 0o777;
    assert_eq!(mode, 0o600);
}

// Regression: re-running init with a different --instance-url but no key used to
// skip the login and leave the previous instance's token in credentials.json, so
// doctor sent it to the new instance and got a confusing 401.

#[test]
fn instance_changed_only_when_an_existing_config_had_another_url() {
    let new = AppConfig { instance_url: "https://new.zitadel.cloud".to_string(), ..existing() };

    assert!(instance_changed(Some(&existing()), &new));
    assert!(!instance_changed(Some(&existing()), &existing()));
    assert!(!instance_changed(None, &new));
}

#[test]
fn a_new_instance_discards_every_stored_login_but_not_app_json() {
    // The service user's and every person's tokens belong to the previous
    // instance (issue #175: there may be many people, not one user file).
    let dir = tempfile::tempdir().unwrap();
    let cli = dir.path().join("zitadel-cli");
    for file in ["credentials-service.json", "users/alice/credentials.json", "users/bob/credentials.json", "app.json"] {
        let path = cli.join(file);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, "{}").unwrap();
    }

    discard_instance_credentials(dir.path()).unwrap();

    assert!(!cli.join("credentials-service.json").exists());
    assert!(!cli.join("users/alice").exists());
    assert!(!cli.join("users/bob").exists());
    assert!(cli.join("app.json").exists());
}

#[test]
fn discarding_with_nothing_stored_is_fine() {
    let dir = tempfile::tempdir().unwrap();

    discard_instance_credentials(dir.path()).unwrap();
}

#[test]
fn unreadable_key_file_names_the_path() {
    let err = read_key_file(std::path::Path::new("/nonexistent/key.json")).unwrap_err();

    assert!(
        matches!(&err, CliError::KeyFileUnreadable { path, .. } if path == "/nonexistent/key.json"),
        "got {err:?}"
    );
    assert!(err.to_string().ends_with("Check the path passed to --key-file."), "got {err}");
}

#[test]
fn invalid_key_file_explains_why_and_names_the_path() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("key.json");
    std::fs::write(&path, r#"{"type":"application","keyId":"k","key":"x","clientId":"c"}"#).unwrap();

    let err = read_key_file(&path).unwrap_err();

    match &err {
        CliError::InvalidKeyFile { path: p, reason } => {
            assert_eq!(p, &path.display().to_string());
            assert!(reason.starts_with("this is a key of type \"application\""), "got {reason}");
        }
        other => panic!("expected InvalidKeyFile, got {other:?}"),
    }
}

// ── init --user-app (issue #195) ──────────────────────────────────────────

fn written(dir: &std::path::Path, client_id: Option<&str>) -> String {
    let path = dir.join("zitadel-cli").join("app.json");
    let config = AppConfig {
        instance_url: "https://acme.zitadel.cloud".to_string(),
        service_user: None,
        client_id: client_id.map(str::to_string),
    };
    write_app_config(&path, &config).unwrap();
    path.display().to_string()
}

#[test]
fn user_app_report_is_the_app_config_check_once_the_native_app_is_set() {
    let dir = tempfile::tempdir().unwrap();
    let path = written(dir.path(), Some("123@cli"));

    let (report, ok) = user_app_report(dir.path());

    assert!(ok);
    assert_eq!(
        report,
        serde_json::json!({"app_config": {
            "status": "ok",
            "path": path,
            "instance_url": "https://acme.zitadel.cloud",
            "service_user_configured": false,
            "native_app_configured": true,
        }})
    );
}

#[test]
fn user_app_report_fails_without_a_native_app_client_id() {
    // Without the Native app nobody can log in: the setup is not done.
    let dir = tempfile::tempdir().unwrap();
    let path = written(dir.path(), None);

    let (report, ok) = user_app_report(dir.path());

    assert!(!ok);
    assert_eq!(report["app_config"]["status"], "error");
    assert_eq!(report["app_config"]["path"], path);
    assert_eq!(
        report["app_config"]["message"],
        "no Native app configured: app.json has no \"client_id\". Run: zitadel init --user-app --client-id <client-id>"
    );
}

#[test]
fn user_app_report_fails_without_app_json() {
    let dir = tempfile::tempdir().unwrap();

    let (report, ok) = user_app_report(dir.path());

    assert!(!ok);
    assert_eq!(report["app_config"]["status"], "error");
}

#[test]
fn user_app_takes_no_user_flag() {
    // --user-app logs nobody in, so naming a person is a mistake to report,
    // not to ignore (checked at runtime: clap can't see a global --user placed
    // before the subcommand).
    assert!(check_user_app_flag(&Identity::Service).is_ok());

    let err = check_user_app_flag(&Identity::User(UserId::parse("alice").unwrap())).unwrap_err();
    assert_eq!(
        err.to_string(),
        "init --user-app sets up the Native app every person logs in with and logs nobody in, so it takes no --user. \
        Run: zitadel init --user-app --client-id <client-id>, then zitadel auth login --user alice; \
        or zitadel init --user alice --client-id <client-id> to do both at once"
    );
}
