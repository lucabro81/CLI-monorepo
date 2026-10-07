//! Handler for the `init` command — guided onboarding for humans.
//!
//! This is the only command in the crate with narrative (non-JSON) output.
//! It is run once per identity per machine (`jira init` for the Service
//! Account, `jira init --user <id>` for the 3LO app every person logs in with,
//! logging that person in).
//!
//! The flow is:
//! 1. Print numbered setup instructions for the identity's credential: a
//!    Service Account at admin.atlassian.com, or a 3LO app at
//!    developer.atlassian.com (`--user <id>`).
//! 2. Read the Client ID and Client Secret — from `--client-id`/`--client-secret`
//!    flags if provided, otherwise from interactive stdin prompts.
//! 3. Write that identity's section of `app.json` via `write_app_config`,
//!    keeping the other section.
//! 4. Log in as that identity (`client_credentials`, or the browser flow with
//!    `--user <id>`) and save its credentials file.
//! 5. Call `doctor::run_doctor` for that identity and print its JSON report.
//!
//! `init --user-app` (issue #195) stops after step 3 for the 3LO app: it writes
//! the "user" section, logs nobody in, and prints `{"app_config": ...}` for that
//! section (`user_app_check`), so the app can be set up where nobody can open a
//! browser and people log in later with `auth login --user <id> --remote`.
//!
//! `write_app_config` is kept as a separate public function so it can be unit-tested
//! in isolation without going through the interactive flow.

use std::io::{self, BufRead, IsTerminal, Write};
use std::path::Path;

use serde_json::{Value, json};

use crate::auth::{self, AppConfig, Identity, OAuthConfig, OAuthConfigError};
use crate::commands::doctor;
use crate::context::{app_config_error, config_dir};
use crate::error::CliError;

const SERVICE_INSTRUCTIONS: &str = "\
=== jira init: Service Account setup (the default identity) ===

Step 1: Go to https://admin.atlassian.com -> Directory -> Service accounts.
Step 2: Create (or select) a service account and give it access to the Jira site.
Step 3: Click \"Create credentials\" -> \"OAuth 2.0\" and select the Jira scopes:
        read:jira-work
        read:jira-user
        write:jira-work
Step 4: Copy the Client ID and Client Secret (shown once).

For a person (commands run with --user <USER_ID>), run `jira init --user <USER_ID>` instead.
";

const USER_INSTRUCTIONS: &str = "\
=== jira init --user: 3LO app setup (every person logs in with it, used with --user) ===

Step 1: Go to https://developer.atlassian.com/console/myapps/
Step 2: Click \"Create\" and choose \"OAuth 2.0 integration\".
Step 3: Give it a name (e.g. \"jira-cli\"), Resource-level access.
Step 4: In the \"Authorization\" section, add callback URL:
        http://localhost:8080/callback
Step 5: In \"Permissions\", add the Jira API scopes:
        read:jira-work
        read:jira-user
        write:jira-work
Step 6: Under \"Settings\", copy the Client ID and Client Secret.
";

/// The section of app.json `init` writes: the Service Account's, or the 3LO app
/// every person logs in with.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppSection {
    Service,
    User,
}

impl From<&Identity> for AppSection {
    fn from(identity: &Identity) -> Self {
        match identity {
            Identity::Service => AppSection::Service,
            Identity::User(_) => AppSection::User,
        }
    }
}

/// Writes `section` of `<config_dir>/jira-cli/app.json`, keeping the
/// other section. A missing file or one in the legacy single-identity
/// format is replaced (the latter with a notice); a file that is not valid JSON is
/// left alone and reported.
/// Creates parent directories if they do not exist.
pub fn write_app_config(
    config_dir: &Path,
    section: AppSection,
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
                Run jira init{} again if the other identity needs them.",
                path.display(),
                match section {
                    AppSection::Service => " --user <USER_ID>",
                    AppSection::User => "",
                }
            );
            AppConfig { service: None, user: None }
        }
        Err(e) => return Err(app_config_error(e, &path)),
    };
    let config = Some(OAuthConfig {
        client_id: client_id.to_string(),
        client_secret: client_secret.to_string(),
        redirect_uri: OAuthConfig::REDIRECT_URI.to_string(),
    });
    match section {
        AppSection::Service => app.service = config,
        AppSection::User => app.user = config,
    }

    // It holds the client secrets: written owner-only (#165).
    let serialized = app.to_json().map_err(|e| CliError::JsonSerialize { reason: e.to_string() })?;
    oauth_user_login::write_secret_file(&path, serialized.as_bytes()).map_err(|e| CliError::SaveCredentialsFailed {
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

/// Prompts for a secret: hidden (no echo) when stdin is a terminal, so a typed
/// secret never lands in scrollback (issue #196); a plain line otherwise, so
/// piped input keeps working.
fn prompt_secret(label: &str) -> Result<String, CliError> {
    read_secret_with(label, io::stdin().is_terminal(), rpassword::prompt_password, prompt)
}

/// [`prompt_secret`] with the terminal check and both readers injected, for tests.
pub(crate) fn read_secret_with(
    label: &str,
    is_terminal: bool,
    hidden: impl FnOnce(String) -> io::Result<String>,
    line: impl FnOnce(&str) -> Result<String, CliError>,
) -> Result<String, CliError> {
    if !is_terminal {
        return line(label);
    }
    let value = hidden(format!("{label}: "))
        .map_err(|e| CliError::IoError { reason: format!("could not read {label}: {e}") })?;
    Ok(value.trim().to_string())
}

/// Runs the init onboarding flow for `identity`.
pub fn run_init(identity: &Identity, client_id: Option<String>, client_secret: Option<String>) -> Result<(), CliError> {
    match identity {
        Identity::Service => println!("{SERVICE_INSTRUCTIONS}"),
        Identity::User(_) => println!("{USER_INSTRUCTIONS}"),
    }

    let client_id = match client_id {
        Some(id) => id,
        None => prompt("Enter Client ID")?,
    };
    let client_secret = match client_secret {
        Some(s) => s,
        None => prompt_secret("Enter Client Secret")?,
    };

    let cfg_dir = config_dir()?;
    write_app_config(&cfg_dir, identity.into(), &client_id, &client_secret)?;
    println!("\napp.json written to {}", auth::app_config_path(&cfg_dir).display());

    let oauth_config = OAuthConfig {
        client_id,
        client_secret,
        redirect_uri: OAuthConfig::REDIRECT_URI.to_string(),
    };
    let credentials = match identity {
        Identity::Service => auth::login_client_credentials(&oauth_config),
        Identity::User(_) => {
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
        Identity::Service => "jira auth whoami".to_string(),
        Identity::User(id) => format!("jira auth whoami --user {id}"),
    };
    println!("\nSetup complete. Run `{whoami}` to verify the identity.");
    Ok(())
}

/// `doctor`'s `app_config` check for the 3LO app's section, the only thing
/// `init --user-app` sets up (no identity to check credentials for yet).
pub fn user_app_check(config_dir: &Path) -> Value {
    let path = auth::app_config_path(config_dir);
    let path_str = path.display().to_string();
    let error = |message: String| json!({"status": "error", "path": path_str, "section": "user", "message": message});
    match AppConfig::load(&path) {
        Ok(app) if app.user.is_some() => json!({"status": "ok", "path": path_str, "section": "user"}),
        Ok(_) => error(
            "app.json has no \"user\" section. Run: jira init --user-app --client-id <ID> --client-secret <SECRET>"
                .to_string(),
        ),
        Err(e) => error(app_config_error(e, &path).to_string()),
    }
}

/// `init --user-app` names no person: refuses `--user <id>`.
pub fn check_user_app_flag(identity: &Identity) -> Result<(), CliError> {
    match identity {
        Identity::Service => Ok(()),
        Identity::User(id) => Err(CliError::UserAppWithUser { id: id.to_string() }),
    }
}

/// Runs `init --user-app`: writes the 3LO app every person logs in with, logs
/// nobody in, and prints its `app_config` check.
pub fn run_init_user_app(
    identity: &Identity,
    client_id: Option<String>,
    client_secret: Option<String>,
) -> Result<(), CliError> {
    check_user_app_flag(identity)?;
    println!("{USER_INSTRUCTIONS}");

    let client_id = match client_id {
        Some(id) => id,
        None => prompt("Enter Client ID")?,
    };
    let client_secret = match client_secret {
        Some(s) => s,
        None => prompt_secret("Enter Client Secret")?,
    };

    let cfg_dir = config_dir()?;
    write_app_config(&cfg_dir, AppSection::User, &client_id, &client_secret)?;
    println!("\napp.json written to {}\n", auth::app_config_path(&cfg_dir).display());

    let check = user_app_check(&cfg_dir);
    let ok = check["status"] == "ok";
    let output = serde_json::to_string_pretty(&json!({"app_config": check}))
        .map_err(|e| CliError::JsonSerialize { reason: e.to_string() })?;
    println!("{output}");
    if !ok {
        return Err(CliError::DoctorCheckFailed);
    }

    println!(
        "\nNobody is logged in yet. Log each person in with `jira auth login --user <USER_ID>` \
        (browser on this machine) or `jira auth login --user <USER_ID> --remote --redirect-uri <URL>` \
        (the person opens the link elsewhere)."
    );
    Ok(())
}

#[cfg(test)]
#[path = "../tests/commands/init_tests.rs"]
mod tests;
