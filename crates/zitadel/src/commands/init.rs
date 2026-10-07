//! Handler for the `init` command: writes `app.json`, logs in as the service
//! user (if configured) or, with `--user <id>`, as that person through the browser,
//! and runs `doctor` for that identity as the final verification.
//!
//! `init --user-app` (issue #195) writes `app.json` the same way but logs nobody
//! in, and prints only the `app_config` check (`user_app_report`): the Native app
//! can be set up where nobody can open a browser, people log in later with
//! `auth login --user <id> --remote`.
//!
//! Flag-driven only (no interactive prompts — an LLM caller can't answer them).
//! Re-running merges with the existing `app.json`: omitted flags keep their
//! current value. Narrative progress goes to stderr; stdout carries only the
//! doctor JSON report.

use std::path::Path;

use serde_json::{Value, json};

use crate::auth::{self, AppConfig, AppConfigError, Identity, ServiceUserKey};
use crate::commands::doctor;
use crate::context::{config_dir, print_json};
use crate::error::CliError;

pub fn run_init(
    identity: &Identity,
    instance_url: Option<&str>,
    key_file: Option<&Path>,
    client_id: Option<&str>,
    select: cli_fields::Select<'_>,
) -> Result<(), CliError> {
    let config_dir = config_dir()?;
    let config = write_config(&config_dir, instance_url, key_file, client_id)?;

    let creds_path = auth::credentials_path(&config_dir, identity);
    let credentials = match identity {
        Identity::Service if config.service_user.is_none() => {
            eprintln!(
                "No service user key configured (--key-file): skipping login. \
                Run zitadel init --user <USER_ID> --client-id <ID> to log in as a person instead."
            );
            None
        }
        Identity::Service => Some(
            auth::login_service_user(&config).map_err(|e| CliError::LoginFailed { reason: e.to_string() })?,
        ),
        Identity::User(id) => {
            eprintln!("Starting the browser login as {id}.");
            Some(
                auth::login_user(&config)
                    .map_err(|e| CliError::UserLoginFailed { reason: e.to_string(), id: id.to_string() })?,
            )
        }
    };
    if let Some(credentials) = credentials {
        auth::save_credentials(&creds_path, &credentials).map_err(|e| {
            CliError::SaveCredentialsFailed {
                path: creds_path.display().to_string(),
                reason: e.to_string(),
            }
        })?;
        eprintln!("Logged in. Credentials saved to {}", creds_path.display());
    }

    let (report, all_ok) = doctor::run_doctor_in(&config_dir, identity);
    // Same exemption as `doctor`: full report unless an explicit --select is given.
    print_json(&report, select.or_all())?;
    if !all_ok {
        return Err(CliError::DoctorCheckFailed);
    }
    Ok(())
}

/// Builds `app.json` from the flags merged with the existing file, writes it, and
/// discards every stored login when the instance URL changed.
fn write_config(
    config_dir: &Path,
    instance_url: Option<&str>,
    key_file: Option<&Path>,
    client_id: Option<&str>,
) -> Result<AppConfig, CliError> {
    let app_path = auth::app_config_path(config_dir);

    let existing = match AppConfig::load(&app_path) {
        Ok(config) => Some(config),
        Err(AppConfigError::NotFound(_)) => None,
        Err(e) => {
            eprintln!("Ignoring the existing invalid {} ({e}).", app_path.display());
            None
        }
    };
    let service_user = key_file.map(read_key_file).transpose()?;
    let previous = existing.clone();
    let config = build_app_config(existing, instance_url, service_user, client_id)?;
    write_app_config(&app_path, &config)?;
    eprintln!("Wrote {}", app_path.display());

    if instance_changed(previous.as_ref(), &config) {
        discard_instance_credentials(config_dir)?;
        eprintln!("Instance URL changed: removed the credentials of the previous instance.");
    }
    Ok(config)
}

/// `doctor`'s `app_config` check, the only one `init --user-app` runs (no identity
/// to check yet), as `{"app_config": ...}`; not ok without a Native app client id,
/// which every person's login needs.
pub(crate) fn user_app_report(config_dir: &Path) -> (Value, bool) {
    let (mut check, config) = doctor::check_app_config(config_dir);
    let ok = match config {
        Some(config) if config.client_id.is_some() => true,
        Some(_) => {
            check["status"] = json!("error");
            check["message"] = json!(
                "no Native app configured: app.json has no \"client_id\". Run: zitadel init --user-app --client-id <client-id>"
            );
            false
        }
        None => false,
    };
    (json!({"app_config": check}), ok)
}

/// `init --user-app` names no person: refuses `--user <id>`.
pub fn check_user_app_flag(identity: &Identity) -> Result<(), CliError> {
    match identity {
        Identity::Service => Ok(()),
        Identity::User(id) => Err(CliError::UserAppWithUser { id: id.to_string() }),
    }
}

/// Runs `init --user-app`: writes `app.json` like `init`, logs nobody in, and
/// prints the `app_config` check.
pub fn run_init_user_app(
    identity: &Identity,
    instance_url: Option<&str>,
    key_file: Option<&Path>,
    client_id: Option<&str>,
    select: cli_fields::Select<'_>,
) -> Result<(), CliError> {
    check_user_app_flag(identity)?;
    let config_dir = config_dir()?;
    write_config(&config_dir, instance_url, key_file, client_id)?;

    let (report, ok) = user_app_report(&config_dir);
    print_json(&report, select.or_all())?;
    if !ok {
        return Err(CliError::DoctorCheckFailed);
    }
    eprintln!(
        "Nobody is logged in. Log each person in with `zitadel auth login --user <USER_ID>` (browser on this \
        machine) or `zitadel auth login --user <USER_ID> --remote --redirect-uri <URL>` (the person opens the \
        link elsewhere)."
    );
    Ok(())
}

pub(crate) fn read_key_file(path: &Path) -> Result<ServiceUserKey, CliError> {
    let raw = std::fs::read_to_string(path).map_err(|e| CliError::KeyFileUnreadable {
        path: path.display().to_string(),
        reason: e.to_string(),
    })?;
    ServiceUserKey::from_key_file(&raw).map_err(|e| CliError::InvalidKeyFile {
        path: path.display().to_string(),
        reason: e.to_string(),
    })
}

/// True when an existing config pointed at a different instance — its stored
/// token belongs to that other instance and must not be reused.
pub(crate) fn instance_changed(previous: Option<&AppConfig>, new: &AppConfig) -> bool {
    previous.is_some_and(|p| p.instance_url != new.instance_url)
}

/// Removes every stored login (the service user's and every person's): they
/// belong to the previous instance.
pub(crate) fn discard_instance_credentials(config_dir: &Path) -> Result<(), CliError> {
    let failed = |e: std::io::Error| CliError::SaveCredentialsFailed {
        path: config_dir.join("zitadel-cli").display().to_string(),
        reason: format!("could not remove the previous instance's credentials: {e}"),
    };
    let users = auth::list_users(config_dir).map_err(failed)?;
    for stale in std::iter::once(Identity::Service).chain(users.into_iter().map(Identity::User)) {
        auth::remove_identity(config_dir, &stale).map_err(failed)?;
    }
    Ok(())
}

/// Merges flags over an existing config; flags win, omitted flags keep the
/// existing value. The instance URL is validated/normalized by `AppConfig::from_json`.
pub(crate) fn build_app_config(
    existing: Option<AppConfig>,
    instance_url: Option<&str>,
    service_user: Option<ServiceUserKey>,
    client_id: Option<&str>,
) -> Result<AppConfig, CliError> {
    let (existing_url, existing_key, existing_client) = match existing {
        Some(c) => (Some(c.instance_url), c.service_user, c.client_id),
        None => (None, None, None),
    };
    let url = instance_url
        .map(str::to_string)
        .or(existing_url)
        .ok_or(CliError::InstanceUrlRequired)?;

    let value = app_config_json(&AppConfig {
        instance_url: url.clone(),
        service_user: service_user.or(existing_key),
        client_id: client_id.map(str::to_string).or(existing_client),
    });
    AppConfig::from_json(&value.to_string()).map_err(|e| match e {
        AppConfigError::InvalidInstanceUrl(_) => CliError::InvalidInstanceUrl { value: url },
        // The value was just built from typed fields; only the URL can be invalid.
        other => CliError::Internal {
            reason: format!("re-parsing the merged app config failed: {other}"),
        },
    })
}

fn app_config_json(config: &AppConfig) -> serde_json::Value {
    let mut value = json!({"instance_url": config.instance_url});
    if let Some(key) = &config.service_user {
        value["service_user"] = json!(key);
    }
    if let Some(client_id) = &config.client_id {
        value["client_id"] = json!(client_id);
    }
    value
}

/// Writes `app.json` atomically with owner-only permissions (it holds the
/// private key), creating its folder; a looser existing file ends up 0600 too.
pub(crate) fn write_app_config(path: &Path, config: &AppConfig) -> Result<(), CliError> {
    let fail = |e: std::io::Error| CliError::WriteAppConfigFailed {
        path: path.display().to_string(),
        reason: e.to_string(),
    };
    let mut contents = serde_json::to_string_pretty(&app_config_json(config)).map_err(|e| {
        CliError::WriteAppConfigFailed {
            path: path.display().to_string(),
            reason: e.to_string(),
        }
    })?;
    contents.push('\n');
    oauth_user_login::write_secret_file(path, contents.as_bytes()).map_err(fail)
}

#[cfg(test)]
#[path = "../tests/commands/init_tests.rs"]
mod tests;
