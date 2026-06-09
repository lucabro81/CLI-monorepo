//! Handlers for the `auth` command group (`auth login`, `auth whoami`).
//!
//! `run_login` drives the full OAuth 2.0 (3LO) + PKCE consent flow: it loads
//! the app credentials from `app.json`, opens the browser, waits for the
//! callback, exchanges the authorization code for tokens, and persists them to
//! `credentials.json`. It is intentionally interactive and human-facing.
//!
//! `run_whoami` makes a single API call to `/rest/api/3/myself` and prints the
//! authenticated user as JSON. It is the quickest sanity-check after login.

use crate::auth;
use crate::context::{authenticated_client, config_dir, load_oauth_config, print_json};
use crate::error::CliError;

/// Runs the OAuth 2.0 login flow and saves credentials to disk.
pub fn run_login() -> Result<(), CliError> {
    let oauth_config = load_oauth_config()?;
    let path = auth::credentials_path(&config_dir()?);
    let credentials = auth::login(&oauth_config).map_err(|e| CliError::LoginFailed {
        reason: e.to_string(),
    })?;
    auth::save_credentials(&path, &credentials).map_err(|e| CliError::SaveCredentialsFailed {
        path: path.display().to_string(),
        reason: e.to_string(),
    })?;
    println!("Logged in. Credentials saved to {}", path.display());
    Ok(())
}

/// Prints the currently authenticated user as JSON.
pub fn run_whoami(select: &[&str]) -> Result<(), CliError> {
    let value = authenticated_client()?.get_myself().map_err(|e| CliError::ApiRequestFailed {
        reason: e.to_string(),
    })?;
    print_json(&value, select)
}
