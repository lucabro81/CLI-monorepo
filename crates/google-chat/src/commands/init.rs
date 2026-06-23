//! Handler for the `init` command — guided onboarding for humans.
//!
//! This is the only command in the crate with narrative (non-JSON) output.
//! It is intended to be run once per machine to get everything configured
//! for the interactive (`--user`) login path.
//!
//! The flow is:
//! 1. Print numbered setup instructions for creating a Google OAuth 2.0
//!    Desktop app client at console.cloud.google.com.
//! 2. Read the Client ID and Client Secret — from `--client-id`/`--client-secret`
//!    flags if provided, otherwise from interactive stdin prompts.
//! 3. Write `app.json` to the XDG config directory via `write_app_config`.
//! 4. Run the interactive OAuth browser login flow (`auth::login`).
//! 5. Call `doctor::run_doctor` and print its JSON report as confirmation.
//!
//! This does not set up the non-interactive domain-wide-delegation flow
//! (`auth login`, no flags) — that requires a Workspace super-admin and is
//! documented separately in README.md. `write_app_config` preserves any
//! existing `service_account` block already in `app.json` rather than
//! clobbering it, so re-running `init` to rotate the OAuth client secret
//! doesn't undo that setup.
//!
//! `write_app_config` is kept as a separate public function so it can be unit-tested
//! in isolation without going through the interactive flow.

use std::io::{self, BufRead, Write};
use std::path::Path;

use serde_json::json;

use crate::auth::{self, OAuthConfig};
use crate::commands::doctor;
use crate::context::config_dir;
use crate::error::CliError;

const INSTRUCTIONS: &str = "\
=== google-chat init: Google OAuth 2.0 app setup ===

Step 1: Go to https://console.cloud.google.com/apis/credentials
Step 2: Enable the Google Chat API for your project (APIs & Services > Library).
Step 3: Configure the OAuth consent screen (APIs & Services > OAuth consent screen):
        - User type: Internal
Step 4: Create an OAuth 2.0 Client ID of type \"Desktop app\".
Step 5: Copy the Client ID and Client Secret.

Note: this sets up the interactive (--user) login only. For non-interactive
agent-driven login (auth login, no flags), a Workspace super-admin must also
set up domain-wide delegation — see README.md \"Setup\" for details.
";

/// Writes `app.json` with the given OAuth credentials to
/// `<config_dir>/google-chat-cli/app.json`. Creates parent directories if
/// they do not exist. Preserves an existing `service_account` block if one is
/// already present in the file, so re-running `init` doesn't undo a
/// hand-configured domain-wide-delegation setup.
pub fn write_app_config(config_dir: &Path, client_id: &str, client_secret: &str) -> Result<(), CliError> {
    let dir = config_dir.join("google-chat-cli");
    std::fs::create_dir_all(&dir).map_err(|e| CliError::SaveCredentialsFailed {
        path: dir.display().to_string(),
        reason: e.to_string(),
    })?;

    let path = dir.join("app.json");

    let existing_service_account = std::fs::read_to_string(&path)
        .ok()
        .and_then(|raw| serde_json::from_str::<serde_json::Value>(&raw).ok())
        .and_then(|v| v.get("service_account").cloned());

    let mut content = json!({
        "client_id": client_id,
        "client_secret": client_secret,
    });
    if let Some(service_account) = existing_service_account {
        content["service_account"] = service_account;
    }

    let serialized = serde_json::to_string_pretty(&content).map_err(|e| CliError::JsonSerialize {
        reason: e.to_string(),
    })?;

    std::fs::write(&path, serialized).map_err(|e| CliError::SaveCredentialsFailed {
        path: path.display().to_string(),
        reason: e.to_string(),
    })
}

/// Prompts the user to enter a value on stdin. Returns the trimmed input.
fn prompt(label: &str) -> Result<String, CliError> {
    print!("{label}: ");
    io::stdout().flush().map_err(|e| CliError::IoError { reason: e.to_string() })?;

    let stdin = io::stdin();
    let line = stdin.lock().lines().next().ok_or_else(|| CliError::IoError {
        reason: "unexpected end of input while reading prompt".to_string(),
    })?.map_err(|e| CliError::IoError { reason: e.to_string() })?;

    Ok(line.trim().to_string())
}

/// Runs the full init onboarding flow.
pub fn run_init(client_id: Option<String>, client_secret: Option<String>) -> Result<(), CliError> {
    println!("{INSTRUCTIONS}");

    let client_id = match client_id {
        Some(id) => id,
        None => prompt("Enter Client ID")?,
    };
    let client_secret = match client_secret {
        Some(s) => s,
        None => prompt("Enter Client Secret")?,
    };

    let cfg_dir = config_dir()?;
    write_app_config(&cfg_dir, &client_id, &client_secret)?;
    println!("\napp.json written to {}", auth::app_config_path(&cfg_dir).display());

    println!("\nStarting OAuth login flow — your browser will open.\n");
    let oauth_config = OAuthConfig {
        client_id,
        client_secret,
        redirect_uri: OAuthConfig::REDIRECT_URI.to_string(),
        service_account: None,
    };
    let credentials = auth::login(&oauth_config).map_err(|e| CliError::LoginFailed {
        reason: e.to_string(),
    })?;
    let creds_path = auth::credentials_path(&cfg_dir);
    auth::save_credentials(&creds_path, &credentials).map_err(|e| {
        CliError::SaveCredentialsFailed {
            path: creds_path.display().to_string(),
            reason: e.to_string(),
        }
    })?;
    println!("Login successful.\n");

    println!("Running doctor check...\n");
    let (report, all_ok) = doctor::run_doctor()?;
    let output = serde_json::to_string_pretty(&report).map_err(|e| CliError::JsonSerialize {
        reason: e.to_string(),
    })?;
    println!("{output}");

    if !all_ok {
        return Err(CliError::DoctorCheckFailed);
    }

    println!("\nSetup complete. Run `google-chat doctor` again any time to verify.");
    Ok(())
}

#[cfg(test)]
#[path = "../tests/commands/init_tests.rs"]
mod tests;
