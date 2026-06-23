//! Handler for the `auth` command group (`auth login`).
//!
//! `run_login` saves credentials to `credentials.json` using one of two grants:
//!
//! - `user = false` (default) — domain-wide delegation: signs a JWT assertion
//!   with the service account's private key from `app.json`, impersonating the
//!   configured Workspace user. No browser, no user interaction.
//! - `user = true` — OAuth 2.0 Authorization Code + PKCE, for a human Google
//!   account: opens the browser, waits for the local callback, exchanges the
//!   authorization code for tokens. Interactive and human-facing.

use crate::auth;
use crate::context::{config_dir, load_oauth_config};
use crate::error::CliError;

/// Runs the OAuth 2.0 login flow and saves credentials to disk.
/// `user = true` runs the interactive Authorization Code + PKCE flow; otherwise
/// runs the non-interactive domain-wide-delegation flow for a service account.
pub fn run_login(user: bool) -> Result<(), CliError> {
    let oauth_config = load_oauth_config()?;
    let path = auth::credentials_path(&config_dir()?);
    let credentials = if user {
        auth::login(&oauth_config)
    } else {
        auth::login_service_account(&oauth_config)
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
