//! Handler for the `init` command: writes `app.json`, logs in as the service
//! user (if configured), and runs `doctor` as the final verification.
//!
//! Flag-driven only (no interactive prompts — an LLM caller can't answer them).
//! Re-running merges with the existing `app.json`: omitted flags keep their
//! current value. Narrative progress goes to stderr; stdout carries only the
//! doctor JSON report.

use std::path::Path;

use serde_json::json;

use crate::auth::{self, AppConfig, AppConfigError, ServiceUserKey};
use crate::commands::doctor;
use crate::context::{config_dir, print_json};
use crate::error::CliError;

pub fn run_init(
    instance_url: Option<&str>,
    key_file: Option<&Path>,
    client_id: Option<&str>,
    select: cli_fields::Select<'_>,
) -> Result<(), CliError> {
    let config_dir = config_dir()?;
    let app_path = auth::app_config_path(&config_dir);

    let existing = match AppConfig::load(&app_path) {
        Ok(config) => Some(config),
        Err(AppConfigError::NotFound(_)) => None,
        Err(e) => {
            eprintln!("Ignoring the existing invalid {} ({e}).", app_path.display());
            None
        }
    };
    let service_user = key_file.map(read_key_file).transpose()?;
    let config = build_app_config(existing, instance_url, service_user, client_id)?;
    write_app_config(&app_path, &config)?;
    eprintln!("Wrote {}", app_path.display());

    if config.service_user.is_some() {
        let creds_path = auth::credentials_path(&config_dir);
        let credentials = auth::login_service_user(&config).map_err(|e| CliError::LoginFailed {
            reason: e.to_string(),
        })?;
        auth::save_credentials(&creds_path, &credentials).map_err(|e| {
            CliError::SaveCredentialsFailed {
                path: creds_path.display().to_string(),
                reason: e.to_string(),
            }
        })?;
        eprintln!("Logged in as the service user. Credentials saved to {}", creds_path.display());
    } else {
        eprintln!("No service user key configured (--key-file): skipping login.");
    }

    let (report, all_ok) = doctor::run_doctor_in(&config_dir);
    // Same exemption as `doctor`: full report unless an explicit --select is given.
    print_json(&report, select.or_all())?;
    if !all_ok {
        return Err(CliError::DoctorCheckFailed);
    }
    Ok(())
}

fn read_key_file(path: &Path) -> Result<ServiceUserKey, CliError> {
    let raw = std::fs::read_to_string(path).map_err(|e| CliError::KeyFileUnreadable {
        path: path.display().to_string(),
        reason: e.to_string(),
    })?;
    ServiceUserKey::from_key_file(&raw).map_err(|e| CliError::InvalidKeyFile {
        path: path.display().to_string(),
        reason: e.to_string(),
    })
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
        other => CliError::WriteAppConfigFailed {
            path: "app.json".to_string(),
            reason: other.to_string(),
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

/// Writes `app.json` with owner-only permissions (it holds the private key),
/// tightening them if the file already existed with looser ones.
pub(crate) fn write_app_config(path: &Path, config: &AppConfig) -> Result<(), CliError> {
    let fail = |e: std::io::Error| CliError::WriteAppConfigFailed {
        path: path.display().to_string(),
        reason: e.to_string(),
    };
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(fail)?;
    }
    let mut contents = serde_json::to_string_pretty(&app_config_json(config)).map_err(|e| {
        CliError::WriteAppConfigFailed {
            path: path.display().to_string(),
            reason: e.to_string(),
        }
    })?;
    contents.push('\n');
    std::fs::write(path, contents).map_err(fail)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600)).map_err(fail)?;
    }
    Ok(())
}

#[cfg(test)]
#[path = "../tests/commands/init_tests.rs"]
mod tests;
