//! Handler for the `auth` command group.
//!
//! `run_login` logs in as the service user (private key JWT) or, with `--user`,
//! as a human via the browser (authorization code + PKCE), and saves the
//! resulting credentials to `credentials.json`. With `--user --remote` it runs
//! the two-step login for a person not at this machine: step 1 prints the
//! authorize URL and stores a pending login, step 2 (`--code --state`) completes
//! it and prints the identity. `run_whoami` prints the authenticated identity.

use serde_json::{Value, json};

use crate::auth::{self, LoginError};
use crate::context::{authenticated_client, client_error_to_cli, config_dir, load_app_config, print_json};
use crate::error::CliError;

/// Which login `auth login` runs, decided by its flags (clap has already
/// rejected the invalid combinations).
#[derive(Debug, PartialEq, Eq)]
pub enum LoginMode {
    ServiceUser,
    UserBrowser,
    RemoteStart { redirect_uri: String },
    RemoteComplete { code: String, state: String },
}

impl LoginMode {
    pub fn from_flags(
        user: bool,
        redirect_uri: Option<String>,
        code: Option<String>,
        state: Option<String>,
    ) -> Self {
        match (user, redirect_uri, code, state) {
            (_, Some(redirect_uri), _, _) => LoginMode::RemoteStart { redirect_uri },
            (_, None, Some(code), Some(state)) => LoginMode::RemoteComplete { code, state },
            (true, ..) => LoginMode::UserBrowser,
            (false, ..) => LoginMode::ServiceUser,
        }
    }
}

pub fn run_login(mode: LoginMode, select: cli_fields::Select<'_>) -> Result<(), CliError> {
    let config = load_app_config()?;
    let dir = config_dir()?;
    let pending_path = auth::pending_login_path(&dir);
    let user_login_failed = |e: LoginError| CliError::UserLoginFailed { reason: e.to_string() };
    let (credentials, remote) = match mode {
        LoginMode::ServiceUser => {
            let credentials = auth::login_service_user(&config)
                .map_err(|e| CliError::LoginFailed { reason: e.to_string() })?;
            (credentials, false)
        }
        LoginMode::UserBrowser => (auth::login_user(&config).map_err(user_login_failed)?, false),
        LoginMode::RemoteStart { redirect_uri } => {
            let (url, pending) =
                auth::start_remote_login(&config, &redirect_uri, &pending_path, auth::now_unix())
                    .map_err(user_login_failed)?;
            // A small object synthesized here: exempt from mandatory --select.
            return print_json(&remote_start_output(&url, &pending), select.or_all());
        }
        LoginMode::RemoteComplete { code, state } => {
            let credentials =
                auth::complete_remote_login(&config, &pending_path, &code, &state, auth::now_unix())
                    .map_err(remote_login_error)?;
            (credentials, true)
        }
    };
    let path = auth::credentials_path(&dir);
    auth::save_credentials(&path, &credentials).map_err(|e| CliError::SaveCredentialsFailed {
        path: path.display().to_string(),
        reason: e.to_string(),
    })?;
    if remote {
        // The caller (e.g. an agent) learns who just logged in.
        return run_whoami(select);
    }
    println!("Logged in. Credentials saved to {}", path.display());
    Ok(())
}

/// Step 1's output. Never includes the PKCE verifier.
pub(crate) fn remote_start_output(authorize_url: &str, pending: &oauth_user_login::PendingLogin) -> Value {
    json!({
        "authorize_url": authorize_url,
        "state": pending.state,
        "expires_at": oauth_user_login::rfc3339_utc(pending.expires_at),
    })
}

/// Maps step 2's failures to an actionable error.
pub(crate) fn remote_login_error(error: LoginError) -> CliError {
    match error {
        LoginError::PendingLogin(e) => CliError::RemoteLoginFailed { reason: e.to_string() },
        LoginError::TokenExchange(detail) => CliError::RemoteLoginFailed {
            reason: format!(
                "ZITADEL refused the code ({detail}). A code is valid once and only for a short time, \
                and the pending login is now used up"
            ),
        },
        other => CliError::UserLoginFailed { reason: other.to_string() },
    }
}

pub fn run_whoami(select: cli_fields::Select<'_>) -> Result<(), CliError> {
    let user = authenticated_client()?
        .get_current_user()
        .map_err(client_error_to_cli)?;
    // Exempt from mandatory --select: a single, small, fixed-shape object.
    print_json(&user, select.or_all())
}

#[cfg(test)]
#[path = "../tests/commands/auth_tests.rs"]
mod tests;
