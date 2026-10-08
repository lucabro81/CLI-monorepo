#![allow(clippy::unwrap_used, clippy::expect_used)]

use super::{
    InitInputs, InitMode, Question, build_app_config, check_user_app_flag, discard_instance_credentials,
    instance_changed, read_key_file, read_secret_with, resolve_init_inputs, user_app_report, write_app_config,
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

// ── interactive init (issue #228) ────────────────────────────────────────

/// Throwaway RSA key generated only for tests (same fixture as `auth_tests`).
const TEST_PRIVATE_KEY: &str = include_str!("../fixtures/test_rsa_private_key.pem");

fn pasted_key_json() -> String {
    serde_json::json!({"type": "serviceaccount", "keyId": "k-1", "key": TEST_PRIVATE_KEY, "userId": "u-1"}).to_string()
}

fn pasted_key() -> ServiceUserKey {
    ServiceUserKey { key_id: "k-1".to_string(), key: TEST_PRIVATE_KEY.to_string(), user_id: "u-1".to_string() }
}

/// Answers each question from `answers` in order and records what was asked.
type Asked = std::rc::Rc<std::cell::RefCell<Vec<Question>>>;

fn answering(answers: Vec<String>) -> (impl FnMut(Question) -> Result<String, CliError>, Asked) {
    let asked = std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
    let log = asked.clone();
    let mut answers = answers.into_iter();
    (move |q| {
        log.borrow_mut().push(q);
        Ok(answers.next().expect("asked more questions than answered"))
    }, asked)
}

fn no_questions(q: Question) -> Result<String, CliError> {
    panic!("must not ask {q:?}")
}

#[test]
fn service_setup_asks_the_instance_url_then_the_key() {
    let (ask, asked) = answering(vec!["https://acme.zitadel.cloud".to_string(), pasted_key_json()]);

    let inputs = resolve_init_inputs(InitMode::Service, None, None, None, None, ask).unwrap();

    assert_eq!(*asked.borrow(), vec![Question::InstanceUrl, Question::ServiceUserKey]);
    assert_eq!(
        inputs,
        InitInputs {
            instance_url: Some("https://acme.zitadel.cloud".to_string()),
            service_user: Some(pasted_key()),
            client_id: None,
        }
    );
}

#[test]
fn the_instance_url_is_asked_only_when_missing() {
    // Changing it discards every login: it stays a deliberate --instance-url.
    let (ask, asked) = answering(vec![pasted_key_json()]);

    let inputs = resolve_init_inputs(InitMode::Service, None, None, None, Some(&existing()), ask).unwrap();

    assert_eq!(*asked.borrow(), vec![Question::ServiceUserKey]);
    assert_eq!(inputs.instance_url, None, "keeps the existing URL");
}

#[test]
fn flags_skip_their_questions() {
    let dir = tempfile::tempdir().unwrap();
    let key_file = dir.path().join("key.json");
    std::fs::write(&key_file, pasted_key_json()).unwrap();

    let inputs = resolve_init_inputs(
        InitMode::Service,
        Some("https://acme.zitadel.cloud"),
        Some(&key_file),
        Some("123@cli"),
        None,
        no_questions,
    )
    .unwrap();

    assert_eq!(
        inputs,
        InitInputs {
            instance_url: Some("https://acme.zitadel.cloud".to_string()),
            service_user: Some(pasted_key()),
            client_id: Some("123@cli".to_string()),
        }
    );
}

#[test]
fn people_setup_asks_the_instance_url_and_the_client_id_never_the_key() {
    for mode in [InitMode::User, InitMode::UserApp] {
        let (ask, asked) = answering(vec!["https://acme.zitadel.cloud".to_string(), "123@cli".to_string()]);

        let inputs = resolve_init_inputs(mode, None, None, None, None, ask).unwrap();

        assert_eq!(*asked.borrow(), vec![Question::InstanceUrl, Question::ClientId], "{mode:?}");
        assert_eq!(
            inputs,
            InitInputs {
                instance_url: Some("https://acme.zitadel.cloud".to_string()),
                service_user: None,
                client_id: Some("123@cli".to_string()),
            }
        );
    }
}

#[test]
fn people_setup_with_an_existing_instance_asks_only_the_client_id() {
    let (ask, asked) = answering(vec!["123@cli".to_string()]);

    resolve_init_inputs(InitMode::UserApp, None, None, None, Some(&existing()), ask).unwrap();

    assert_eq!(*asked.borrow(), vec![Question::ClientId]);
}

#[test]
fn answers_are_trimmed() {
    let (ask, _) = answering(vec!["  https://acme.zitadel.cloud \n".to_string(), " 123@cli ".to_string()]);

    let inputs = resolve_init_inputs(InitMode::UserApp, None, None, None, None, ask).unwrap();

    assert_eq!(inputs.instance_url.as_deref(), Some("https://acme.zitadel.cloud"));
    assert_eq!(inputs.client_id.as_deref(), Some("123@cli"));
}

#[test]
fn an_empty_answer_writes_nothing_and_names_the_flag() {
    for (mode, answers, field, flag) in [
        (InitMode::Service, vec![String::new()], "Instance URL", "--instance-url"),
        (InitMode::Service, vec!["https://acme.zitadel.cloud".to_string(), " ".to_string()], "Service user key", "--key-file <path>"),
        (InitMode::UserApp, vec!["https://acme.zitadel.cloud".to_string(), String::new()], "Native app client ID", "--client-id"),
    ] {
        let (ask, _) = answering(answers);

        let err = resolve_init_inputs(mode, None, None, None, None, ask).unwrap_err();

        assert_eq!(
            err.to_string(),
            format!(
                "{field} is empty: nothing was written. Run the same zitadel init command again and type it, \
                or pass it with {flag}"
            )
        );
    }
}

#[test]
fn an_invalid_pasted_key_says_to_paste_the_whole_file() {
    for (pasted, reason) in [
        ("{\"type\":\"serviceaccount\"".to_string(), "not a valid JSON key file"),
        (
            serde_json::json!({"type": "application", "keyId": "k", "key": "x", "clientId": "c"}).to_string(),
            "this is a key of type \"application\"",
        ),
    ] {
        let (ask, _) = answering(vec!["https://acme.zitadel.cloud".to_string(), pasted]);

        let err = resolve_init_inputs(InitMode::Service, None, None, None, None, ask).unwrap_err();

        let message = err.to_string();
        assert!(message.starts_with("the pasted service user key is not valid ("), "{message}");
        assert!(message.contains(reason), "{message}");
        assert!(
            message.ends_with(
                "Paste the whole content of the key JSON downloaded from the console \
                (Users > Service Users > <user> > Keys > New, type JSON), or pass the file with --key-file <path>"
            ),
            "{message}"
        );
    }
}

#[test]
fn on_a_terminal_the_key_is_read_hidden() {
    let secret = read_secret_with(
        "Paste the key",
        true,
        |prompt| {
            assert_eq!(prompt, "Paste the key: ");
            Ok("  {\"k\":1} \n".to_string())
        },
        |_| panic!("the visible line reader must not be used on a terminal"),
    )
    .unwrap();

    assert_eq!(secret, "{\"k\":1}");
}

#[test]
fn without_a_terminal_the_key_is_a_plain_line() {
    // Piped input (scripts) keeps working.
    let secret = read_secret_with(
        "Paste the key",
        false,
        |_| panic!("no terminal: nothing to hide the input on"),
        |label| {
            assert_eq!(label, "Paste the key");
            Ok("{\"k\":1}".to_string())
        },
    )
    .unwrap();

    assert_eq!(secret, "{\"k\":1}");
}

#[test]
fn a_failed_hidden_read_is_an_io_error() {
    let err = read_secret_with("Paste the key", true, |_| Err(std::io::Error::other("no tty")), |_| panic!("not used"))
        .unwrap_err();

    assert_eq!(err.to_string(), "I/O error: could not read Paste the key: no tty");
}

