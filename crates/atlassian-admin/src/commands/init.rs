//! Handler for the `init` command — non-interactive onboarding.
//!
//! Deliberately different from every other crate's `init` (see crate
//! CLAUDE.md's "init's non-interactive design"): this crate never falls back
//! to an interactive stdin prompt for the API key, since it's a long-lived,
//! org-wide-privileged secret and typing it into a terminal risks it landing
//! in scrollback or session logs.
//!
//! - `init --api-key <KEY> --org-id <ID>` (both provided) — writes `app.json`
//!   directly, then runs `doctor` as live verification, mirroring other
//!   crates' `init`.
//! - `init` (either flag omitted) — writes `app.json` as an empty skeleton
//!   only if it doesn't already exist (never clobbers real credentials), and
//!   prints the exact path to paste real values into by hand. No `doctor`
//!   run — there's nothing live to verify yet.
//!
//! `write_app_config` is kept as a separate public function so it can be unit-tested
//! in isolation.

use std::path::Path;

use serde_json::json;

use crate::commands::doctor;
use crate::context::config_dir;
use crate::error::CliError;

/// Writes `app.json` with the given API key/org id to
/// `<config_dir>/atlassian-admin-cli/app.json`. Creates parent directories if
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

    std::fs::write(&path, serialized).map_err(|e| CliError::SaveConfigFailed {
        path: path.display().to_string(),
        reason: e.to_string(),
    })
}

/// Runs the onboarding flow. Both `--api-key`/`--org-id` provided together
/// writes and verifies; anything else (both omitted, or only one given —
/// treated the same as neither, rather than silently writing a half-real
/// config) creates a skeleton file to fill in by hand.
pub fn run_init(api_key: Option<String>, org_id: Option<String>) -> Result<(), CliError> {
    let cfg_dir = config_dir()?;
    let app_json_path = crate::auth::app_config_path(&cfg_dir);

    if let (Some(api_key), Some(org_id)) = (api_key, org_id) {
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
        This command does not prompt for credentials interactively — an \
        Organization API key is too sensitive to risk landing in terminal \
        scrollback or session logs. Open the file above and paste in your \
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
