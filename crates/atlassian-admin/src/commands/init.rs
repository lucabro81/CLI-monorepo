//! Handler for the `init` command.
//!
//! The Organization API key is a long-lived, org-wide-privileged secret, so it
//! is never echoed (see crate CLAUDE.md's "init and the API key"):
//!
//! - `init --api-key <KEY> --org-id <ID>` (both provided) — writes `app.json`
//!   directly, then runs `doctor` as live verification, mirroring other
//!   crates' `init`.
//! - `init` on a terminal (issue #196) — prompts for whatever the flags didn't
//!   give: the org id in clear, the API key hidden (no echo, so it never lands
//!   in scrollback); then the same write + `doctor` as above.
//! - `init` without a terminal (either flag omitted) — writes `app.json` as an
//!   empty skeleton only if it doesn't already exist (never clobbers real
//!   credentials), and prints the exact path to paste real values into by hand.
//!   No `doctor` run — there's nothing live to verify yet.
//!
//! `write_app_config` is kept as a separate public function so it can be unit-tested
//! in isolation.

use std::io::{self, BufRead, IsTerminal, Write};
use std::path::Path;

use serde_json::json;

use crate::commands::doctor;
use crate::context::config_dir;
use crate::error::CliError;

/// Writes `app.json` with the given API key/org id to
/// `<config_dir>/atlassian-admin-cli/app.json` (atomically, mode 0600).
/// Creates parent directories if
/// they do not exist. Overwrites any existing file — callers that must not
/// clobber an existing file should check for its existence first.
pub fn write_app_config(config_dir: &Path, api_key: &str, org_id: &str) -> Result<(), CliError> {
    let dir = config_dir.join("atlassian-admin-cli");
    std::fs::create_dir_all(&dir).map_err(|e| CliError::SaveConfigFailed {
        path: dir.display().to_string(),
        reason: e.to_string(),
    })?;

    let path = dir.join("app.json");
    let content = json!({
        "api_key": api_key,
        "org_id": org_id,
    });
    let serialized = serde_json::to_string_pretty(&content).map_err(|e| CliError::JsonSerialize {
        reason: e.to_string(),
    })?;

    // It holds the org-wide API key: written owner-only (#218, as #165 did for the other CLIs).
    oauth_user_login::write_secret_file(&path, serialized.as_bytes()).map_err(|e| CliError::SaveConfigFailed {
        path: path.display().to_string(),
        reason: e.to_string(),
    })
}

/// The API key and org id to write: both flags; or, on a terminal, the flags
/// completed by prompts (`visible` for the org id, `hidden` for the key).
/// `None` off a terminal unless both flags are given: only one is treated the
/// same as neither (the skeleton file), rather than writing a half-real config.
pub(crate) fn resolve_credentials(
    api_key: Option<String>,
    org_id: Option<String>,
    is_terminal: bool,
    visible: impl FnOnce(&str) -> Result<String, CliError>,
    hidden: impl FnOnce(&str) -> Result<String, CliError>,
) -> Result<Option<(String, String)>, CliError> {
    match (api_key, org_id) {
        (Some(api_key), Some(org_id)) => Ok(Some((api_key, org_id))),
        (api_key, org_id) if is_terminal => {
            let org_id = match org_id {
                Some(org_id) => org_id,
                None => non_empty(visible("Organization ID")?, "Organization ID", "--org-id")?,
            };
            let api_key = match api_key {
                Some(api_key) => api_key,
                None => non_empty(hidden("Organization API key")?, "Organization API key", "--api-key")?,
            };
            Ok(Some((api_key, org_id)))
        }
        _ => Ok(None),
    }
}

/// An empty answer (Enter alone) must not overwrite app.json with an empty field.
fn non_empty(value: String, field: &'static str, flag: &'static str) -> Result<String, CliError> {
    if value.is_empty() {
        return Err(CliError::EmptyInput { field, flag });
    }
    Ok(value)
}

/// Reads one visible line from stdin after printing `label`.
fn prompt(label: &str) -> Result<String, CliError> {
    print!("{label}: ");
    io::stdout().flush().map_err(|e| CliError::IoError { reason: e.to_string() })?;
    let line = io::stdin()
        .lock()
        .lines()
        .next()
        .ok_or_else(|| CliError::IoError { reason: format!("unexpected end of input while reading {label}") })?
        .map_err(|e| CliError::IoError { reason: e.to_string() })?;
    Ok(line.trim().to_string())
}

/// Reads a secret from the terminal with echo off.
fn prompt_hidden(label: &str) -> Result<String, CliError> {
    read_hidden_with(label, rpassword::prompt_password)
}

/// [`prompt_hidden`] with the reader injected, for tests.
pub(crate) fn read_hidden_with(
    label: &str,
    hidden: impl FnOnce(String) -> io::Result<String>,
) -> Result<String, CliError> {
    let value = hidden(format!("{label}: ")).map_err(|e| CliError::IoError {
        reason: format!(
            "could not read {label}: {e}. Run atlassian-admin init from a terminal, or pass --api-key and --org-id"
        ),
    })?;
    Ok(value.trim().to_string())
}

/// Runs the onboarding flow: writes and verifies when [`resolve_credentials`]
/// has both values, otherwise creates a skeleton file to fill in by hand.
pub fn run_init(api_key: Option<String>, org_id: Option<String>) -> Result<(), CliError> {
    let cfg_dir = config_dir()?;
    let app_json_path = crate::auth::app_config_path(&cfg_dir);

    let credentials = resolve_credentials(api_key, org_id, io::stdin().is_terminal(), prompt, prompt_hidden)?;
    if let Some((api_key, org_id)) = credentials {
        write_app_config(&cfg_dir, &api_key, &org_id)?;
        println!("app.json written to {}", app_json_path.display());

        println!("\nRunning doctor check...\n");
        let (report, all_ok) = doctor::run_doctor()?;
        let output = serde_json::to_string_pretty(&report).map_err(|e| CliError::JsonSerialize {
            reason: e.to_string(),
        })?;
        println!("{output}");

        if !all_ok {
            return Err(CliError::DoctorCheckFailed);
        }

        println!("\nSetup complete. Run `atlassian-admin user get --account-id <id>` to verify a lookup.");
        return Ok(());
    }

    if app_json_path.exists() {
        println!(
            "app.json already exists at {} — left untouched.\n\n\
            Edit it directly if you need to change your credentials:\n\
            {{\"api_key\": \"...\", \"org_id\": \"...\"}}\n\n\
            Then run `atlassian-admin doctor` to verify.",
            app_json_path.display()
        );
        return Ok(());
    }

    write_app_config(&cfg_dir, "", "")?;
    println!(
        "app.json created at {}\n\n\
        Not run on a terminal, so nothing was asked (on a terminal init prompts, \
        with the API key hidden). Open the file above and paste in your \
        real values:\n\
        {{\"api_key\": \"...\", \"org_id\": \"...\"}}\n\n\
        Then run `atlassian-admin doctor` to verify.",
        app_json_path.display()
    );
    Ok(())
}

#[cfg(test)]
#[path = "../tests/commands/init_tests.rs"]
mod tests;
