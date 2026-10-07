//! Handler for the `auth` command group.
//!
//! `run_login` logs in as the service user (private key JWT) or, with `--user <id>`,
//! as that person via the browser (authorization code + PKCE), and saves the
//! resulting credentials to that identity's file (`credentials-service.json` /
//! `users/<id>/credentials.json`), leaving every other identity logged in. With
//! `--user <id> --remote` it runs the two-step login for a person not at this
//! machine: step 1 prints the authorize URL and stores a pending login in that
//! person's folder, step 2 (`--code --state`) completes it and prints the
//! identity. `run_whoami` prints the account the selected identity acts as;
//! `run_logout` removes its stored login (local only).

use serde_json::{Value, json};

use crate::auth::{self, Identity, LoginError, UserId};
use crate::context::{authenticated_client, client_error_to_cli, config_dir, load_app_config, login_command, print_json};
use crate::error::CliError;

/// Which login `auth login` runs, decided by its flags (clap has already
/// rejected the invalid combinations).
#[derive(Debug, PartialEq, Eq)]
pub enum LoginMode {
    ServiceUser,
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
                Ok(LoginMode::ServiceUser)
            };
        };
        let id = id.clone();
        Ok(match (redirect_uri, code, state) {
            (Some(redirect_uri), ..) => LoginMode::RemoteStart { id, redirect_uri },
            (None, Some(code), Some(state)) => LoginMode::RemoteComplete { id, code, state },
            _ => LoginMode::UserBrowser(id),
        })
    }

    /// The identity this login sets up, i.e. the credentials file it writes.
    pub fn identity(&self) -> Identity {
        match self {
            LoginMode::ServiceUser => Identity::Service,
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
    let config = load_app_config()?;
    let dir = config_dir()?;
    let (credentials, remote) = match mode {
        LoginMode::ServiceUser => {
            let credentials = auth::login_service_user(&config)
                .map_err(|e| CliError::LoginFailed { reason: e.to_string() })?;
            (credentials, false)
        }
        LoginMode::UserBrowser(id) => {
            let credentials = auth::login_user(&config)
                .map_err(|e| CliError::UserLoginFailed { reason: e.to_string(), id: id.to_string() })?;
            (credentials, false)
        }
        LoginMode::RemoteStart { id, redirect_uri } => {
            let pending_path = auth::pending_login_path(&dir, &id);
            let (url, pending) = auth::start_remote_login(&config, &redirect_uri, &pending_path, auth::now_unix())
                .map_err(|e| remote_login_error(e, &id))?;
            // A small object synthesized here: exempt from mandatory --select.
            return print_json(&remote_start_output(&url, &pending), select.or_all());
        }
        LoginMode::RemoteComplete { id, code, state } => {
            let pending_path = auth::pending_login_path(&dir, &id);
            let credentials =
                auth::complete_remote_login(&config, &pending_path, &code, &state, auth::now_unix())
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
            "ZITADEL refused the code ({detail}). A code is valid once and only for a short time, \
            and the pending login is now used up"
        ),
        // The pending login may already be consumed: restart from step 1.
        other => other.to_string(),
    };
    CliError::RemoteLoginFailed { reason, id: id.to_string() }
}

/// Prints the account `identity` acts as.
pub fn run_whoami(select: cli_fields::Select<'_>, identity: &Identity) -> Result<(), CliError> {
    let user = authenticated_client(identity)?
        .get_current_user()
        .map_err(client_error_to_cli)?;
    // Exempt from mandatory --select: a single, small, fixed-shape object.
    print_json(&user, select.or_all())
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
