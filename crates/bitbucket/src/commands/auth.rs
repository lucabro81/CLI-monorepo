//! Handlers for the `auth` command group (`auth login`, `auth whoami`).
//!
//! `run_login` saves credentials to `credentials.json`, in one of these modes:
//!
//! - default — OAuth 2.0 `client_credentials`: exchanges the OAuth consumer's
//!   `client_id`/`client_secret` from `app.json` directly for an access token.
//!   No browser, no user interaction. Acts as the OAuth app.
//! - `--user` — OAuth 2.0 `authorization_code`: opens the browser, waits for
//!   the local callback, exchanges the code for tokens. Acts as the human who
//!   approved the consent page.
//! - `--user --remote` — the same grant in two steps, for a person not at this
//!   machine: step 1 prints the consent URL and stores a pending login (state
//!   only), step 2 (`--code --state`) completes it and prints the identity.
//!
//! `run_whoami` makes a single API call to `/2.0/user` and prints the
//! authenticated account as JSON. It is the quickest sanity-check after login.

use serde_json::{json, Value};

use crate::auth::{self, LoginError};
use crate::context::{authenticated_client, config_dir, load_oauth_config, print_json};
use crate::error::CliError;

/// Which login `auth login` runs, decided by its flags (clap has already
/// rejected the invalid combinations).
#[derive(Debug, PartialEq, Eq)]
pub enum LoginMode {
    App,
    UserBrowser,
    RemoteStart,
    RemoteComplete { code: String, state: String },
}

impl LoginMode {
    pub fn from_flags(user: bool, remote: bool, code: Option<String>, state: Option<String>) -> Self {
        match (user, remote, code, state) {
            (_, true, _, _) => LoginMode::RemoteStart,
            (_, false, Some(code), Some(state)) => LoginMode::RemoteComplete { code, state },
            (true, ..) => LoginMode::UserBrowser,
            (false, ..) => LoginMode::App,
        }
    }
}

/// Runs the login selected by `mode` and saves credentials to disk.
pub fn run_login(mode: LoginMode, select: cli_fields::Select<'_>) -> Result<(), CliError> {
    let oauth_config = load_oauth_config()?;
    let dir = config_dir()?;
    let pending_path = auth::pending_login_path(&dir);
    let login_failed = |e: LoginError| CliError::LoginFailed { reason: e.to_string() };
    let (credentials, remote) = match mode {
        LoginMode::App => (auth::login_client_credentials(&oauth_config).map_err(login_failed)?, false),
        LoginMode::UserBrowser => (auth::login(&oauth_config).map_err(login_failed)?, false),
        LoginMode::RemoteStart => {
            let (url, pending) =
                auth::start_remote_login(&oauth_config, &pending_path, auth::now_unix()).map_err(login_failed)?;
            // A small object synthesized here: exempt from mandatory --select.
            return print_json(&remote_start_output(&url, &pending), select.or_all());
        }
        LoginMode::RemoteComplete { code, state } => {
            let credentials =
                auth::complete_remote_login(&oauth_config, &pending_path, &code, &state, auth::now_unix())
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

/// Step 1's output.
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
                "Bitbucket refused the code ({detail}). A code is valid once and only for a short time, \
                and the pending login is now used up"
            ),
        },
        other => CliError::LoginFailed { reason: other.to_string() },
    }
}

/// Prints the currently authenticated account as JSON.
/// Exempt from the mandatory --select requirement: an identity check, small fixed shape.
pub fn run_whoami(select: cli_fields::Select<'_>) -> Result<(), CliError> {
    let value = authenticated_client()?
        .get_current_user()
        .map_err(|e| CliError::ApiRequestFailed {
            reason: e.to_string(),
        })?;
    print_json(&value, select.or_all())
}

#[cfg(test)]
#[path = "../tests/commands/auth_tests.rs"]
mod tests;
