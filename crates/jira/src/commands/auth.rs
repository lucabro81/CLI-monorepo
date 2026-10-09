//! Handlers for the `auth` command group (`auth login`, `auth whoami`, `auth logout`).
//!
//! `run_login` runs one of these modes. Each belongs to one identity
//! (`LoginMode::identity`) and saves only to that identity's credentials file
//! (`credentials-service.json` / `users/<id>/credentials.json`), so every other
//! identity stays logged in:
//!
//! - default — OAuth 2.0 `client_credentials`, for the Service Account:
//!   exchanges `client_id`/`client_secret` from `app.json`'s `service` section
//!   directly for an access token. No browser, no user interaction.
//! - `--user <id>` — OAuth 2.0 (3LO) + PKCE, for a person's Atlassian account, with
//!   `app.json`'s `user` section: opens the browser, waits for the local
//!   callback, exchanges the authorization code for tokens. Interactive and
//!   human-facing.
//! - `--user <id> --remote` — the same 3LO grant in two steps, for a person not at
//!   this machine: step 1 prints the authorize URL and stores a pending login in
//!   that person's folder, step 2 (`--code --state`) completes it and prints the identity.
//!
//! `run_whoami` makes a single API call to `/rest/api/3/myself` as the selected
//! identity and prints that account as JSON. It is the quickest sanity-check after login.
//!
//! `run_logout` removes the selected identity's stored login (local only).

use serde_json::{json, Value};

use crate::auth::{self, Identity, LoginError, UserId};
use crate::context::{authenticated_client, config_dir, load_oauth_config, login_command, print_json};
use crate::error::CliError;

/// Which login `auth login` runs, decided by its flags (clap has already
/// rejected the invalid combinations).
#[derive(Debug, PartialEq, Eq)]
pub enum LoginMode {
    ServiceAccount,
    UserBrowser(UserId),
    RemoteStart { id: UserId, redirect_uri: String },
    RemoteComplete { id: UserId, code: String, state: String },
}

impl LoginMode {
    /// Clap enforces every flag combination except "remote needs --user <id>": a
    /// global --user written before the subcommand is invisible to clap's
    /// `requires`, so that one is checked here.
    pub fn from_flags(
        identity: &Identity,
        redirect_uri: Option<String>,
        code: Option<String>,
        state: Option<String>,
    ) -> Result<Self, CliError> {
        let Identity::User(id) = identity else {
            return if redirect_uri.is_some() || code.is_some() {
                Err(CliError::RemoteLoginNeedsUser)
            } else {
                Ok(LoginMode::ServiceAccount)
            };
        };
        let id = id.clone();
        Ok(match (redirect_uri, code, state) {
            (Some(redirect_uri), ..) => LoginMode::RemoteStart { id, redirect_uri },
            (None, Some(code), Some(state)) => LoginMode::RemoteComplete { id, code, state },
            _ => LoginMode::UserBrowser(id),
        })
    }

    /// The identity this login sets up: its app.json section and credentials file.
    pub fn identity(&self) -> Identity {
        match self {
            LoginMode::ServiceAccount => Identity::Service,
            LoginMode::UserBrowser(id) | LoginMode::RemoteStart { id, .. } | LoginMode::RemoteComplete { id, .. } => {
                Identity::User(id.clone())
            }
        }
    }
}

/// Runs the login selected by `mode` and saves credentials to that identity's file,
/// leaving every other identity's credentials untouched.
pub fn run_login(mode: LoginMode, select: cli_fields::Select<'_>) -> Result<(), CliError> {
    let identity = mode.identity();
    let oauth_config = load_oauth_config(&identity)?;
    let dir = config_dir()?;
    let login_failed = |e: LoginError| CliError::LoginFailed { reason: e.to_string() };
    let (credentials, remote) = match mode {
        LoginMode::ServiceAccount => (auth::login_client_credentials(&oauth_config).map_err(login_failed)?, false),
        LoginMode::UserBrowser(_) => (auth::login(&oauth_config).map_err(login_failed)?, false),
        LoginMode::RemoteStart { id, redirect_uri } => {
            let pending_path = auth::pending_login_path(&dir, &id);
            let (url, pending) = auth::start_remote_login(&oauth_config, &redirect_uri, &pending_path)
                .map_err(|e| remote_login_error(e, &id))?;
            // A small object synthesized here: exempt from mandatory --select.
            return print_json(&remote_start_output(&url, &pending), select.or_all());
        }
        LoginMode::RemoteComplete { id, code, state } => {
            let pending_path = auth::pending_login_path(&dir, &id);
            let credentials = auth::complete_remote_login(&oauth_config, &pending_path, &code, &state)
                .map_err(|e| remote_login_error(e, &id))?;
            (credentials, true)
        }
    };
    let path = auth::credentials_path(&dir, &identity);
    auth::save_credentials(&path, &credentials).map_err(|e| CliError::SaveCredentialsFailed {
        path: path.display().to_string(),
        reason: e.to_string(),
    })?;
    if remote {
        // The caller (e.g. an agent) learns who just logged in.
        return run_whoami(select, &identity);
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

/// Maps a remote login's failures to an error naming the person's step 1.
pub(crate) fn remote_login_error(error: LoginError, id: &UserId) -> CliError {
    let reason = match error {
        LoginError::PendingLogin(e) => e.to_string(),
        LoginError::TokenExchange(detail) | LoginError::TokenRejected(detail) => format!(
            "Atlassian refused the code ({detail}). A code is valid once and only for a short time, \
            and the pending login is now used up"
        ),
        // The pending login may already be consumed: restart from step 1.
        other => other.to_string(),
    };
    CliError::RemoteLoginFailed { reason, id: id.to_string() }
}

/// Prints the account `identity` acts as, as JSON.
/// Exempt from the mandatory --select requirement: an identity check, small fixed shape.
pub fn run_whoami(select: cli_fields::Select<'_>, identity: &Identity) -> Result<(), CliError> {
    let value = authenticated_client(identity)?.get_myself().map_err(whoami_error)?;
    print_json(&value, select.or_all())
}

/// A failed renewal after a 401 reaches the user as is (a person's expired
/// login exits 3, issue #240); any other client error keeps whoami's message.
pub(crate) fn whoami_error(e: crate::client::ClientError) -> CliError {
    match e {
        crate::client::ClientError::Renewal(error) => *error,
        other => CliError::ApiRequestFailed { reason: other.to_string() },
    }
}

/// Removes `identity`'s stored login and prints which one.
/// Exempt from the mandatory --select requirement: a confirmation synthesized here.
pub fn run_logout(select: cli_fields::Select<'_>, identity: &Identity) -> Result<(), CliError> {
    let removed = auth::remove_identity(&config_dir()?, identity)
        .map_err(|e| CliError::IoError { reason: e.to_string() })?;
    if !removed {
        return Err(CliError::NothingToLogOut { label: identity.label(), login: login_command(identity) });
    }
    print_json(&json!({"logged_out": identity.label()}), select.or_all())
}

#[cfg(test)]
#[path = "../tests/commands/auth_tests.rs"]
mod tests;
