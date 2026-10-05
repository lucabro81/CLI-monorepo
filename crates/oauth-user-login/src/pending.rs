//! The pending login of a two-step (`--remote`) user login: step 1 stores
//! `state`, the PKCE verifier and the redirect URI; step 2 takes them back,
//! checking `state` and expiry, and consumes the file so `state` is single-use.

use std::path::Path;

/// How long a pending login stays valid.
pub const PENDING_LOGIN_TTL_SECS: u64 = 600;

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct PendingLogin {
    pub state: String,
    /// `None` for providers without PKCE (Bitbucket).
    pub code_verifier: Option<String>,
    /// Sent again, unchanged, with the code exchange.
    pub redirect_uri: String,
    /// Unix timestamp (seconds).
    pub expires_at: u64,
}

/// What `doctor` reports about a pending login.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PendingLoginStatus {
    pub expires_at: u64,
    pub expired: bool,
}

#[derive(Debug, PartialEq, Eq, thiserror::Error)]
pub enum PendingLoginError {
    #[error("there is no pending remote login in this config folder (none was started, or it was already completed)")]
    NotFound,
    #[error(
        "the state does not match the pending remote login (the link is from an older attempt, or \
        the state was mistyped). Use the code and state of the latest link"
    )]
    StateMismatch,
    #[error("the pending remote login expired (it is valid for 10 minutes after it was started)")]
    Expired,
    #[error("the pending remote login file is unreadable ({0})")]
    Corrupt(String),
    #[error("could not write or remove the pending remote login file ({0})")]
    Io(String),
}

impl PendingLogin {
    /// A fresh pending login: random `state`, a PKCE verifier when `with_pkce`,
    /// and an expiry [`PENDING_LOGIN_TTL_SECS`] after `now`.
    pub fn new(redirect_uri: &str, with_pkce: bool, now: u64) -> Self {
        PendingLogin {
            state: crate::generate_state(),
            code_verifier: with_pkce.then(crate::generate_code_verifier),
            redirect_uri: redirect_uri.to_string(),
            expires_at: now + PENDING_LOGIN_TTL_SECS,
        }
    }

    /// The S256 challenge for the authorize URL, `None` without PKCE.
    pub fn code_challenge(&self) -> Option<String> {
        self.code_verifier.as_deref().map(crate::code_challenge)
    }

    /// Writes the pending login with owner-only permissions (it holds the PKCE
    /// verifier), replacing any earlier one and creating parent directories.
    pub fn save(&self, path: &Path) -> Result<(), PendingLoginError> {
        use std::io::Write;

        let io = |e: std::io::Error| PendingLoginError::Io(e.to_string());
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(io)?;
        }
        // Serializing this plain struct cannot fail; mapped rather than unwrapped.
        let json = serde_json::to_string_pretty(self).map_err(|e| PendingLoginError::Io(e.to_string()))?;

        let mut options = std::fs::OpenOptions::new();
        options.write(true).create(true).truncate(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
            // New files are created 0600; an existing one loses group/other bits first.
            options.mode(0o600);
            if let Ok(metadata) = std::fs::metadata(path) {
                let mode = metadata.permissions().mode();
                if mode & 0o077 != 0 {
                    std::fs::set_permissions(path, std::fs::Permissions::from_mode(mode & 0o700))
                        .map_err(io)?;
                }
            }
        }
        options.open(path).and_then(|mut file| file.write_all(json.as_bytes())).map_err(io)
    }
}

fn read(path: &Path) -> Result<Option<PendingLogin>, PendingLoginError> {
    match std::fs::read_to_string(path) {
        Ok(raw) => serde_json::from_str(&raw)
            .map(Some)
            .map_err(|e| PendingLoginError::Corrupt(e.to_string())),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(PendingLoginError::Io(e.to_string())),
    }
}

fn remove(path: &Path) -> Result<(), PendingLoginError> {
    std::fs::remove_file(path).map_err(|e| PendingLoginError::Io(e.to_string()))
}

/// Takes the pending login at `path` for `state`. On success the file is
/// removed before returning, so `state` is single-use even if the code
/// exchange then fails. A mismatched `state` leaves the file in place (a stale
/// link must not burn a live login); an expired one is removed.
pub fn take_pending_login(path: &Path, state: &str, now: u64) -> Result<PendingLogin, PendingLoginError> {
    let pending = read(path)?.ok_or(PendingLoginError::NotFound)?;
    if pending.state != state {
        return Err(PendingLoginError::StateMismatch);
    }
    remove(path)?;
    if now >= pending.expires_at {
        return Err(PendingLoginError::Expired);
    }
    Ok(pending)
}

/// `None` when no pending login exists.
pub fn pending_login_status(path: &Path, now: u64) -> Result<Option<PendingLoginStatus>, PendingLoginError> {
    Ok(read(path)?.map(|pending| PendingLoginStatus {
        expires_at: pending.expires_at,
        expired: now >= pending.expires_at,
    }))
}

/// Formats a Unix timestamp as RFC 3339 in UTC, e.g. `2026-10-05T19:34:25Z`.
pub fn rfc3339_utc(secs: u64) -> String {
    let (days, rem) = (secs / 86_400, secs % 86_400);
    // Civil date from days since 1970-01-01 (Howard Hinnant's algorithm).
    let z = days + 719_468;
    let era = z / 146_097;
    let day_of_era = z - era * 146_097;
    let year_of_era = (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let mp = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = year_of_era + era * 400 + u64::from(month <= 2);
    format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}Z",
        rem / 3_600,
        rem % 3_600 / 60,
        rem % 60
    )
}

#[cfg(test)]
#[path = "tests/pending_tests.rs"]
mod tests;
