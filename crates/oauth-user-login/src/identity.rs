//! The two identities every CLI stores side by side (issue #164).

use std::path::{Path, PathBuf};

/// Which of the two stored identities a command acts as (issue #164): the
/// service identity (`client_credentials`, the default) or the human who
/// logged in with `auth login --user`. Each has its own `app.json` section and
/// its own credentials file, so logging in as one never touches the other.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Identity {
    Service,
    User,
}

impl Identity {
    /// Maps the global `--user` flag to the identity it selects.
    pub fn from_user_flag(user: bool) -> Self {
        if user {
            Identity::User
        } else {
            Identity::Service
        }
    }

    /// This identity's credentials file:
    /// `<config_dir>/<cli_dir>/credentials-service.json` or `credentials-user.json`.
    pub fn credentials_path(self, config_dir: &Path, cli_dir: &str) -> PathBuf {
        let file = match self {
            Identity::Service => "credentials-service.json",
            Identity::User => "credentials-user.json",
        };
        config_dir.join(cli_dir).join(file)
    }
}

#[cfg(test)]
#[path = "tests/identity_tests.rs"]
mod tests;
