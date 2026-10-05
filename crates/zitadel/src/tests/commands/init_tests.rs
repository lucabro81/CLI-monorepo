#![allow(clippy::unwrap_used, clippy::expect_used)]

use super::{build_app_config, discard_credentials, instance_changed, write_app_config};
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
fn discard_credentials_removes_the_file() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("credentials.json");
    std::fs::write(&path, "{}").unwrap();

    discard_credentials(&path).unwrap();

    assert!(!path.exists());
}

#[test]
fn discard_credentials_tolerates_a_missing_file() {
    let dir = tempfile::tempdir().unwrap();

    discard_credentials(&dir.path().join("credentials.json")).unwrap();
}
