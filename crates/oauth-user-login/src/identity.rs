//! The identities every CLI stores side by side: one service identity (issue
//! #164) and any number of humans, each named by the caller (issue #175).

use std::path::{Path, PathBuf};

/// The caller's name for a human, given with `--user <id>`. It names the
/// person's folder under `users/`, so it is a lowercase slug: 1-64 characters
/// out of `a-z`, `0-9`, `.`, `_`, `-`, `:`, starting with a letter or digit (no
/// `..`, no `/`, no hidden folders, no case collisions).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct UserId(String);

impl UserId {
    /// Validates `value`; the error is a full sentence for clap to show.
    pub fn parse(value: &str) -> Result<Self, String> {
        let mut chars = value.chars();
        let first_ok = chars.next().is_some_and(|c| c.is_ascii_lowercase() || c.is_ascii_digit());
        let rest_ok =
            chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, '.' | '_' | '-' | ':'));
        if first_ok && rest_ok && value.len() <= 64 {
            Ok(UserId(value.to_string()))
        } else {
            Err(format!(
                "\"{value}\" is not a valid user id: it must be a lowercase slug of 1-64 characters \
                (a-z, 0-9, '.', '_', '-', ':') starting with a letter or digit (examples: jane.doe, chat:u123)"
            ))
        }
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for UserId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// Which stored identity a command acts as: the service identity (the
/// default, a non-interactive grant) or the human logged in with
/// `auth login --user <id>`. Each has its own credentials file, so logging in
/// as one never touches another.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Identity {
    Service,
    User(UserId),
}

impl Identity {
    /// Maps the global `--user <id>` flag to the identity it selects.
    pub fn from_user_flag(user: Option<UserId>) -> Self {
        user.map_or(Identity::Service, Identity::User)
    }

    /// `service` or `user:<id>`, as `doctor` and `auth logout` print it.
    pub fn label(&self) -> String {
        match self {
            Identity::Service => "service".to_string(),
            Identity::User(id) => format!("user:{id}"),
        }
    }

    /// This identity's credentials file:
    /// `<config_dir>/<cli_dir>/credentials-service.json` or
    /// `<config_dir>/<cli_dir>/users/<id>/credentials.json`.
    pub fn credentials_path(&self, config_dir: &Path, cli_dir: &str) -> PathBuf {
        match self {
            Identity::Service => config_dir.join(cli_dir).join("credentials-service.json"),
            Identity::User(id) => user_dir(config_dir, cli_dir, id).join("credentials.json"),
        }
    }
}

fn users_dir(config_dir: &Path, cli_dir: &str) -> PathBuf {
    config_dir.join(cli_dir).join("users")
}

fn user_dir(config_dir: &Path, cli_dir: &str, id: &UserId) -> PathBuf {
    users_dir(config_dir, cli_dir).join(id.as_str())
}

/// A person's pending remote login: `<config_dir>/<cli_dir>/users/<id>/pending-login.json`.
/// One per person, so two people can be mid-login at the same time.
pub fn pending_login_path(config_dir: &Path, cli_dir: &str, id: &UserId) -> PathBuf {
    user_dir(config_dir, cli_dir, id).join("pending-login.json")
}

/// The people logged in (a `users/<id>/credentials.json` exists), sorted.
/// Folders whose name is not a valid id were not made by the CLI and are skipped.
pub fn list_users(config_dir: &Path, cli_dir: &str) -> std::io::Result<Vec<UserId>> {
    let entries = match std::fs::read_dir(users_dir(config_dir, cli_dir)) {
        Ok(entries) => entries,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(e),
    };
    let mut users = Vec::new();
    for entry in entries {
        let entry = entry?;
        let Some(id) = entry.file_name().to_str().and_then(|name| UserId::parse(name).ok()) else {
            continue;
        };
        if entry.path().join("credentials.json").is_file() {
            users.push(id);
        }
    }
    users.sort();
    Ok(users)
}

/// Credentials files from earlier layouts that still exist (never read): the
/// pre-#164 `credentials.json` and the single-human `credentials-user.json` of
/// #164. `doctor` reports them so they can be deleted.
pub fn legacy_credentials_files(config_dir: &Path, cli_dir: &str) -> Vec<&'static str> {
    ["credentials.json", "credentials-user.json"]
        .into_iter()
        .filter(|name| config_dir.join(cli_dir).join(name).exists())
        .collect()
}

/// Removes `identity`'s stored login (`auth logout`): a person's whole
/// `users/<id>/` folder (credentials and any pending remote login), or the
/// service credentials file. `false` when there was nothing to remove.
pub fn remove_identity(config_dir: &Path, cli_dir: &str, identity: &Identity) -> std::io::Result<bool> {
    let result = match identity {
        Identity::Service => std::fs::remove_file(identity.credentials_path(config_dir, cli_dir)),
        Identity::User(id) => std::fs::remove_dir_all(user_dir(config_dir, cli_dir, id)),
    };
    match result {
        Ok(()) => Ok(true),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(e) => Err(e),
    }
}

/// Process exit code of every CLI when the selected identity needs a new
/// login (issue #194): no stored login, or a person's refresh token refused.
/// Every other failure exits 1 (clap usage errors: 2), so a caller can start a
/// login without parsing the message.
pub const NOT_LOGGED_IN_EXIT_CODE: u8 = 3;

/// True when a token endpoint's answer (`status`, response `body`) means the
/// grant itself was refused (400, 401, 403 — RFC 6749 errors such as
/// `invalid_grant`), so only a new login helps. False for transient failures
/// (408, 429, 5xx) or anything else, where retrying may work, and for
/// `invalid_client`: the app's own credentials were refused, and a new login
/// through the same app would fail the same way.
pub fn token_request_rejected(status: u16, body: &str) -> bool {
    let client_refused = serde_json::from_str::<serde_json::Value>(body)
        .is_ok_and(|value| value.get("error").and_then(serde_json::Value::as_str) == Some("invalid_client"));
    matches!(status, 400 | 401 | 403) && !client_refused
}

#[cfg(test)]
#[path = "tests/identity_tests.rs"]
mod tests;
