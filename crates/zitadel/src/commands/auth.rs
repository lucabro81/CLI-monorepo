//! Handler for the `auth` command group.
//!
//! `run_login` logs in as the service user (private key JWT) or, with `--user`,
//! as a human via the browser (authorization code + PKCE), and saves the
//! resulting credentials to `credentials.json`. `run_whoami` prints the
//! authenticated identity.

use crate::auth;
use crate::context::{authenticated_client, client_error_to_cli, config_dir, load_app_config, print_json};
use crate::error::CliError;

pub fn run_login(user: bool) -> Result<(), CliError> {
    let config = load_app_config()?;
    let path = auth::credentials_path(&config_dir()?);
    let credentials = if user {
        auth::login_user(&config).map_err(|e| CliError::UserLoginFailed {
            reason: e.to_string(),
        })
    } else {
        auth::login_service_user(&config).map_err(|e| CliError::LoginFailed {
            reason: e.to_string(),
        })
    }?;
    auth::save_credentials(&path, &credentials).map_err(|e| CliError::SaveCredentialsFailed {
        path: path.display().to_string(),
        reason: e.to_string(),
    })?;
    println!("Logged in. Credentials saved to {}", path.display());
    Ok(())
}

pub fn run_whoami(select: cli_fields::Select<'_>) -> Result<(), CliError> {
    let user = authenticated_client()?
        .get_current_user()
        .map_err(client_error_to_cli)?;
    // Exempt from mandatory --select: a single, small, fixed-shape object.
    print_json(&user, select.or_all())
}
