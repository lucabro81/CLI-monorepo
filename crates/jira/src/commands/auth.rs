//! Handlers for the `auth` command group (`auth login`, `auth whoami`).
//!
//! `run_login` saves credentials to `credentials.json` using one of two grants:
//!
//! - `user = false` (default) — OAuth 2.0 `client_credentials`, for a service
//!   account: exchanges `client_id`/`client_secret` from `app.json` directly for
//!   an access token. No browser, no user interaction.
//! - `user = true` — OAuth 2.0 (3LO) + PKCE, for a human Atlassian account: opens
//!   the browser, waits for the local callback, exchanges the authorization code
//!   for tokens. Interactive and human-facing.
//!
//! `run_whoami` makes a single API call to `/rest/api/3/myself` and prints the
//! authenticated user as JSON. It is the quickest sanity-check after login.

use crate::auth;
use crate::context::{authenticated_client, config_dir, load_oauth_config, print_json};
use crate::error::CliError;

/// Runs the OAuth 2.0 login flow and saves credentials to disk.
/// `user = true` runs the interactive 3LO + PKCE flow; otherwise runs the
/// non-interactive `client_credentials` flow for a service account.
pub fn run_login(user: bool) -> Result<(), CliError> {
    let oauth_config = load_oauth_config()?;
    let path = auth::credentials_path(&config_dir()?);
    let credentials = if user {
        auth::login(&oauth_config)
    } else {
        auth::login_client_credentials(&oauth_config)
    }
    .map_err(|e| CliError::LoginFailed {
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
/// Exempt from the mandatory --select requirement: an identity check, small fixed shape.
pub fn run_whoami(select: cli_fields::Select<'_>) -> Result<(), CliError> {
    let value = authenticated_client()?.get_myself().map_err(|e| CliError::ApiRequestFailed {
        reason: e.to_string(),
    })?;
    print_json(&value, select.or_all())
}
