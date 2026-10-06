//! Handlers for the `auth` command group (`auth login`, `auth whoami`).
//!
//! `run_login` runs one of these modes. Each belongs to one identity
//! (`LoginMode::identity`) and saves only to that identity's credentials file
//! (`credentials-service.json` / `credentials-user.json`), so the other
//! identity stays logged in:
//!
//! - default — OAuth 2.0 `client_credentials`, for the Service Account:
//!   exchanges `client_id`/`client_secret` from `app.json`'s `service` section
//!   directly for an access token. No browser, no user interaction.
//! - `--user` — OAuth 2.0 (3LO) + PKCE, for a human Atlassian account, with
//!   `app.json`'s `user` section: opens the browser, waits for the local
//!   callback, exchanges the authorization code for tokens. Interactive and
//!   human-facing.
//! - `--user --remote` — the same 3LO grant in two steps, for a person not at
//!   this machine: step 1 prints the authorize URL and stores a pending login,
//!   step 2 (`--code --state`) completes it and prints the identity.
//!
//! `run_whoami` makes a single API call to `/wiki/rest/api/user/current` as
//! the selected identity and prints that account as JSON. It is the quickest sanity-check
//! after login.

use serde_json::{json, Value};

use crate::auth::{self, Identity, LoginError};
use crate::context::{authenticated_client, config_dir, load_oauth_config, print_json};
use crate::error::CliError;

/// Which login `auth login` runs, decided by its flags (clap has already
/// rejected the invalid combinations).
#[derive(Debug, PartialEq, Eq)]
pub enum LoginMode {
    ServiceAccount,
    UserBrowser,
    RemoteStart { redirect_uri: String },
    RemoteComplete { code: String, state: String },
}

impl LoginMode {
    /// Clap enforces every flag combination except "remote needs --user": a
    /// global --user written before the subcommand is invisible to clap's
    /// `requires`, so that one is checked here.
    pub fn from_flags(
        identity: Identity,
        redirect_uri: Option<String>,
        code: Option<String>,
        state: Option<String>,
    ) -> Result<Self, CliError> {
        match (identity, redirect_uri, code, state) {
            (Identity::Service, Some(_), ..) | (Identity::Service, None, Some(_), _) => {
                Err(CliError::RemoteLoginNeedsUser)
            }
            (_, Some(redirect_uri), _, _) => Ok(LoginMode::RemoteStart { redirect_uri }),
            (_, None, Some(code), Some(state)) => Ok(LoginMode::RemoteComplete { code, state }),
            (Identity::User, ..) => Ok(LoginMode::UserBrowser),
            (Identity::Service, ..) => Ok(LoginMode::ServiceAccount),
        }
    }

    /// The identity this login sets up: its app.json section and credentials file.
    pub fn identity(&self) -> Identity {
        match self {
            LoginMode::ServiceAccount => Identity::Service,
            LoginMode::UserBrowser | LoginMode::RemoteStart { .. } | LoginMode::RemoteComplete { .. } => {
                Identity::User
            }
        }
    }
}

/// Runs the login selected by `mode` and saves credentials to that identity's file,
/// leaving the other identity's credentials untouched.
pub fn run_login(mode: LoginMode, select: cli_fields::Select<'_>) -> Result<(), CliError> {
    let identity = mode.identity();
    let oauth_config = load_oauth_config(identity)?;
    let dir = config_dir()?;
    let pending_path = auth::pending_login_path(&dir);
    let login_failed = |e: LoginError| CliError::LoginFailed { reason: e.to_string() };
    let (credentials, remote) = match mode {
        LoginMode::ServiceAccount => (auth::login_client_credentials(&oauth_config).map_err(login_failed)?, false),
        LoginMode::UserBrowser => (auth::login(&oauth_config).map_err(login_failed)?, false),
        LoginMode::RemoteStart { redirect_uri } => {
            let (url, pending) =
                auth::start_remote_login(&oauth_config, &redirect_uri, &pending_path).map_err(remote_login_error)?;
            // A small object synthesized here: exempt from mandatory --select.
            return print_json(&remote_start_output(&url, &pending), select.or_all());
        }
        LoginMode::RemoteComplete { code, state } => {
            let credentials = auth::complete_remote_login(&oauth_config, &pending_path, &code, &state)
                .map_err(remote_login_error)?;
            (credentials, true)
        }
    };
    let path = auth::credentials_path(&dir, identity);
    auth::save_credentials(&path, &credentials).map_err(|e| CliError::SaveCredentialsFailed {
        path: path.display().to_string(),
        reason: e.to_string(),
    })?;
    if remote {
        // The caller (e.g. an agent) learns who just logged in.
        return run_whoami(select, identity);
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
                "Atlassian refused the code ({detail}). A code is valid once and only for a short time, \
                and the pending login is now used up"
            ),
        },
        // The pending login may already be consumed: restart from step 1.
        other => CliError::RemoteLoginFailed { reason: other.to_string() },
    }
}

/// Prints the account `identity` acts as, as JSON.
/// Exempt from the mandatory --select requirement: an identity check, small fixed shape.
pub fn run_whoami(select: cli_fields::Select<'_>, identity: Identity) -> Result<(), CliError> {
    let value = authenticated_client(identity)?
        .get_current_user()
        .map_err(|e| CliError::ApiRequestFailed {
            reason: e.to_string(),
        })?;
    print_json(&value, select.or_all())
}

#[cfg(test)]
#[path = "../tests/commands/auth_tests.rs"]
mod tests;
