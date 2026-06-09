use std::io::{self, BufRead, Write};
use std::path::Path;

use serde_json::json;

use crate::auth::{self, OAuthConfig};
use crate::context::config_dir;
use crate::doctor;
use crate::error::CliError;

const INSTRUCTIONS: &str = "\
=== jira init: Atlassian OAuth 2.0 app setup ===

Step 1: Go to https://developer.atlassian.com/console/myapps/
Step 2: Click \"Create\" and choose \"OAuth 2.0 integration\".
Step 3: Give it a name (e.g. \"jira-cli\").
Step 4: In the \"Authorization\" section, add callback URL:
        http://localhost:8080/callback
Step 5: In \"Permissions\", add the Jira API scopes:
        read:jira-work
        read:jira-user
        write:jira-work
        offline_access
Step 6: Under \"Settings\", copy the Client ID and Client Secret.
";

/// Writes `app.json` with the given client credentials to `<config_dir>/jira-cli/app.json`.
/// Creates parent directories if they do not exist.
pub fn write_app_config(config_dir: &Path, client_id: &str, client_secret: &str) -> Result<(), CliError> {
    let dir = config_dir.join("jira-cli");
    std::fs::create_dir_all(&dir).map_err(|e| CliError::SaveCredentialsFailed {
        path: dir.display().to_string(),
        reason: e.to_string(),
    })?;

    let path = dir.join("app.json");
    let content = json!({
        "client_id": client_id,
        "client_secret": client_secret,
    });
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

/// Runs the full init flow.
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
    println!("\napp.json written to {}", cfg_dir.join("jira-cli").join("app.json").display());

    // Run login flow.
    println!("\nStarting OAuth login flow — your browser will open.\n");
    let oauth_config = OAuthConfig {
        client_id,
        client_secret,
        redirect_uri: OAuthConfig::REDIRECT_URI.to_string(),
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

    // Final verification.
    println!("Running doctor check...\n");
    let (report, all_ok) = doctor::run_doctor()?;
    let output = serde_json::to_string_pretty(&report).map_err(|e| CliError::JsonSerialize {
        reason: e.to_string(),
    })?;
    println!("{output}");

    if !all_ok {
        return Err(CliError::DoctorCheckFailed);
    }

    println!("\nSetup complete. Run `jira auth whoami` to verify your identity.");
    Ok(())
}

#[cfg(test)]
#[path = "init_tests.rs"]
mod tests;
