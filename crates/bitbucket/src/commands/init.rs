//! Handler for the `init` command — guided onboarding for humans.
//!
//! This is the only command in the crate with narrative (non-JSON) output.
//! It is run once per identity per machine (`bitbucket init` for the OAuth
//! app, `bitbucket init --user` for the human).
//!
//! The flow is:
//! 1. Print numbered setup instructions for creating a Bitbucket OAuth consumer.
//! 2. Read the Key (`client_id`) and Secret (`client_secret`) — from
//!    `--client-id`/`--client-secret` flags if provided, otherwise from
//!    interactive stdin prompts.
//! 3. Write that identity's section of `app.json` via `write_app_config`,
//!    keeping the other section.
//! 4. Log in as that identity: the `client_credentials` exchange (no browser),
//!    or the browser consent with `--user`.
//! 5. Call `doctor::run_doctor` for that identity and print its JSON report.
//!
//! `write_app_config` is kept as a separate public function so it can be unit-tested
//! in isolation without going through the interactive flow.

use std::io::{self, BufRead, Write};
use std::path::Path;

use crate::auth::{self, AppConfig, Identity, OAuthConfig, OAuthConfigError};
use crate::commands::doctor;
use crate::context::{app_config_error, config_dir};
use crate::error::CliError;

const INSTRUCTIONS: &str = "\
=== bitbucket init: OAuth consumer setup ===

Step 1: Go to your Bitbucket workspace -> Settings -> OAuth consumers -> Add consumer.
Step 2: Give it a name (e.g. \"bitbucket-cli\").
Step 3: Set the callback URL to http://localhost:8080/callback (used only by
        `auth login --user`; the default login ignores it).
Step 4: Grant the permissions your commands need (e.g. Account: Read,
        Repositories: Read/Write, Pull requests: Read/Write).
Step 5: Save, then copy the consumer's Key (client_id) and Secret (client_secret).

`bitbucket init` sets up the OAuth app identity (the default); `bitbucket init
--user` sets up the human identity used with --user. The same consumer can
serve both.
";

/// Writes `identity`'s section of `<config_dir>/bitbucket-cli/app.json`, keeping the
/// other identity's section. A missing file or one in the legacy single-identity
/// format is replaced (the latter with a notice); a file that is not valid JSON is
/// left alone and reported.
/// Creates parent directories if they do not exist.
pub fn write_app_config(
    config_dir: &Path,
    identity: Identity,
    client_id: &str,
    client_secret: &str,
) -> Result<(), CliError> {
    let path = auth::app_config_path(config_dir);
    let mut app = match AppConfig::load(&path) {
        Ok(app) => app,
        Err(OAuthConfigError::NotFound(_)) => AppConfig { service: None, user: None },
        Err(OAuthConfigError::LegacyFormat) => {
            // Its one client_id/client_secret pair can't be assigned to an
            // identity, so it is dropped: say so, the other identity may need it.
            println!(
                "Replacing the old single-identity {}: its client_id/client_secret are discarded. \
                Run bitbucket init{} again if the other identity needs them.",
                path.display(),
                match identity {
                    Identity::Service => " --user",
                    Identity::User => "",
                }
            );
            AppConfig { service: None, user: None }
        }
        Err(e) => return Err(app_config_error(e, &path)),
    };
    let section = Some(OAuthConfig {
        client_id: client_id.to_string(),
        client_secret: client_secret.to_string(),
    });
    match identity {
        Identity::Service => app.service = section,
        Identity::User => app.user = section,
    }

    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| CliError::SaveCredentialsFailed {
            path: dir.display().to_string(),
            reason: e.to_string(),
        })?;
    }
    let serialized = app.to_json().map_err(|e| CliError::JsonSerialize { reason: e.to_string() })?;
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

/// Runs the init onboarding flow for `identity`.
pub fn run_init(identity: Identity, client_id: Option<String>, client_secret: Option<String>) -> Result<(), CliError> {
    println!("{INSTRUCTIONS}");

    let client_id = match client_id {
        Some(id) => id,
        None => prompt("Enter Key (client_id)")?,
    };
    let client_secret = match client_secret {
        Some(s) => s,
        None => prompt("Enter Secret (client_secret)")?,
    };

    let cfg_dir = config_dir()?;
    write_app_config(&cfg_dir, identity, &client_id, &client_secret)?;
    println!("\napp.json written to {}", auth::app_config_path(&cfg_dir).display());

    let oauth_config = OAuthConfig { client_id, client_secret };
    let credentials = match identity {
        Identity::Service => {
            println!("\nRequesting access token via client_credentials...\n");
            auth::login_client_credentials(&oauth_config)
        }
        Identity::User => {
            println!("\nStarting OAuth login flow — your browser will open.\n");
            auth::login(&oauth_config)
        }
    }
    .map_err(|e| CliError::LoginFailed { reason: e.to_string() })?;
    let creds_path = auth::credentials_path(&cfg_dir, identity);
    auth::save_credentials(&creds_path, &credentials).map_err(|e| {
        CliError::SaveCredentialsFailed {
            path: creds_path.display().to_string(),
            reason: e.to_string(),
        }
    })?;
    println!("Login successful.\n");

    println!("Running doctor check...\n");
    let (report, all_ok) = doctor::run_doctor(identity)?;
    let output = serde_json::to_string_pretty(&report).map_err(|e| CliError::JsonSerialize {
        reason: e.to_string(),
    })?;
    println!("{output}");

    if !all_ok {
        return Err(CliError::DoctorCheckFailed);
    }

    let whoami = match identity {
        Identity::Service => "bitbucket auth whoami",
        Identity::User => "bitbucket auth whoami --user",
    };
    println!("\nSetup complete. Run `{whoami}` to verify the identity.");
    Ok(())
}

#[cfg(test)]
#[path = "../tests/commands/init_tests.rs"]
mod tests;
