//! Handler for the `init` command — guided onboarding for humans.
//!
//! This is the only command in the crate with narrative (non-JSON) output.
//! It is run once per identity per machine (`confluence init` for the Service
//! Account, `confluence init --user` for the human's 3LO app).
//!
//! The flow is:
//! 1. Print numbered setup instructions for the identity's credential: a
//!    Service Account at admin.atlassian.com, or a 3LO app at
//!    developer.atlassian.com (`--user`).
//! 2. Read the Client ID and Client Secret — from `--client-id`/`--client-secret`
//!    flags if provided, otherwise from interactive stdin prompts.
//! 3. Write that identity's section of `app.json` via `write_app_config`,
//!    keeping the other section.
//! 4. Log in as that identity (`client_credentials`, or the browser flow with
//!    `--user`) and save its credentials file.
//! 5. Call `doctor::run_doctor` for that identity and print its JSON report.
//!
//! `write_app_config` is kept as a separate public function so it can be unit-tested
//! in isolation without going through the interactive flow.

use std::io::{self, BufRead, Write};
use std::path::Path;

use crate::auth::{self, AppConfig, Identity, OAuthConfig, OAuthConfigError, SCOPES};
use crate::commands::doctor;
use crate::context::{app_config_error, config_dir};
use crate::error::CliError;

const SERVICE_INSTRUCTIONS: &str = "\
=== confluence init: Service Account setup (the default identity) ===

Step 1: Go to https://admin.atlassian.com -> Directory -> Service accounts.
Step 2: Create (or select) a service account and give it access to the Confluence site.
Step 3: Click \"Create credentials\" -> \"OAuth 2.0\" and select the Confluence
        scopes listed below (\"Scopes to add\").
Step 4: Copy the Client ID and Client Secret (shown once).

For a human identity (commands run with --user), run `confluence init --user` instead.
";

const USER_INSTRUCTIONS: &str = "\
=== confluence init --user: 3LO app setup (the human identity, used with --user) ===

Step 1: Go to https://developer.atlassian.com/console/myapps/
Step 2: Click \"Create\" and choose \"OAuth 2.0 integration\".
Step 3: Give it a name (e.g. \"confluence-cli\"), Resource-level access.
Step 4: In the \"Authorization\" section, add callback URL:
        http://localhost:8080/callback
Step 5: In \"Permissions\" -> Confluence API, add the scopes listed below
        (\"Scopes to add\"): read:confluence-user and search:confluence under
        Classic scopes, the others under Granular scopes. offline_access is
        requested by the CLI itself and needs no setting.
Step 6: Under \"Settings\", copy the Client ID and Client Secret.
";

/// Writes `identity`'s section of `<config_dir>/confluence-cli/app.json`, keeping the
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
                Run confluence init{} again if the other identity needs them.",
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
        redirect_uri: OAuthConfig::REDIRECT_URI.to_string(),
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
    match identity {
        Identity::Service => println!("{SERVICE_INSTRUCTIONS}"),
        Identity::User => println!("{USER_INSTRUCTIONS}"),
    }
    println!("Scopes to add: {SCOPES}\n");

    let client_id = match client_id {
        Some(id) => id,
        None => prompt("Enter Client ID")?,
    };
    let client_secret = match client_secret {
        Some(s) => s,
        None => prompt("Enter Client Secret")?,
    };

    let cfg_dir = config_dir()?;
    write_app_config(&cfg_dir, identity, &client_id, &client_secret)?;
    println!("\napp.json written to {}", auth::app_config_path(&cfg_dir).display());

    let oauth_config = OAuthConfig {
        client_id,
        client_secret,
        redirect_uri: OAuthConfig::REDIRECT_URI.to_string(),
    };
    let credentials = match identity {
        Identity::Service => auth::login_client_credentials(&oauth_config),
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
        Identity::Service => "confluence auth whoami",
        Identity::User => "confluence auth whoami --user",
    };
    println!("\nSetup complete. Run `{whoami}` to verify the identity.");
    Ok(())
}

#[cfg(test)]
#[path = "../tests/commands/init_tests.rs"]
mod tests;
