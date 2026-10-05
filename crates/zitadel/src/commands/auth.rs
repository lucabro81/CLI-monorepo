//! Handler for the `auth` command group.
//!
//! `run_login` logs in as the service user (private key JWT) and saves the
//! resulting credentials to `credentials.json`.

use crate::auth;
use crate::context::{config_dir, load_app_config};
use crate::error::CliError;

pub fn run_login() -> Result<(), CliError> {
    let config = load_app_config()?;
    let path = auth::credentials_path(&config_dir()?);
    let credentials = auth::login_service_user(&config).map_err(|e| CliError::LoginFailed {
        reason: e.to_string(),
    })?;
    auth::save_credentials(&path, &credentials).map_err(|e| CliError::SaveCredentialsFailed {
        path: path.display().to_string(),
        reason: e.to_string(),
    })?;
    println!("Logged in. Credentials saved to {}", path.display());
    Ok(())
}
