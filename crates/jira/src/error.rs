//! Top-level error type for the `jira` CLI.
//!
//! `CliError` is the single error type that surfaces to the user. Every
//! variant carries a self-contained message — what went wrong and what the
//! caller (human or LLM) should do to fix it. Messages are plain text with
//! no colors, symbols, or formatting.
//!
//! Internal module errors (`LoginError`, `ClientError`, `OAuthConfigError`)
//! are mapped to `CliError` at the `run()` boundary in `main.rs` or in the
//! relevant command handler. They are never exposed directly to the user.

use thiserror::Error;

/// Top-level CLI error. Every variant carries a self-contained message:
/// what went wrong and what the caller should do to fix it.
/// No colors, symbols, or formatting — output is designed to be read by an LLM.
#[derive(Debug, Error)]
pub enum CliError {
    #[error(
        "app credentials file not found at {path}. Create it with: \
        jira init --client-id <ID> --client-secret <SECRET> (Service Account, the default identity) \
        and/or jira init --user <USER_ID> --client-id <ID> --client-secret <SECRET> (3LO app, used with --user <USER_ID>)"
    )]
    AppConfigNotFound { path: String },

    #[error(
        "app credentials file at {path} is not valid: {reason}. Expected format: \
        {{\"service\": {{\"client_id\": \"...\", \"client_secret\": \"...\"}}, \
        \"user\": {{\"client_id\": \"...\", \"client_secret\": \"...\"}}}} (either section may be omitted). \
        Fix or delete the file, then run jira init"
    )]
    AppConfigInvalid { path: String, reason: String },

    #[error(
        "app.json at {path} uses the old single-identity format (client_id at top level). \
        Recreate it: jira init --client-id <ID> --client-secret <SECRET> for the Service Account, \
        and jira init --user <USER_ID> --client-id <ID> --client-secret <SECRET> for the 3LO app used with --user"
    )]
    AppConfigLegacy { path: String },

    #[error(
        "app.json at {path} has no \"service\" section (the Service Account credential used without --user). \
        Run: jira init --client-id <ID> --client-secret <SECRET>"
    )]
    ServiceAppMissing { path: String },

    #[error(
        "app.json at {path} has no \"user\" section (the 3LO app every person logs in with). \
        Run: jira init --user {id} --client-id <ID> --client-secret <SECRET>, \
        or jira init --user-app --client-id <ID> --client-secret <SECRET> to set it up without logging anyone in"
    )]
    UserAppMissing { path: String, id: String },

    #[error(
        "init --user-app sets up the 3LO app every person logs in with and logs nobody in, so it takes no --user. \
        Run: jira init --user-app --client-id <ID> --client-secret <SECRET>, then jira auth login --user {id}; \
        or jira init --user {id} --client-id <ID> --client-secret <SECRET> to do both at once"
    )]
    UserAppWithUser { id: String },

    #[error(
        "no home directory found — cannot resolve config path. \
        Set the XDG_CONFIG_HOME environment variable explicitly."
    )]
    NoHomeDirectory,

    #[error(
        "not logged in as the service account. Run: jira auth login. \
        To act as a person logged in with jira auth login --user <USER_ID>, pass --user <USER_ID> instead"
    )]
    NotAuthenticatedService,

    #[error(
        "user {id} is not logged in. Run: jira auth login --user {id} \
        (the person must approve the login in a browser)"
    )]
    NotAuthenticatedUser { id: String },

    #[error(
        "failed to refresh authentication token: {reason}. \
        The session may have been revoked. Run: {login}"
    )]
    TokenRefreshFailed { reason: String, login: String },

    #[error(
        "the login of user {id} is no longer valid ({reason}): the refresh token expired or was revoked. \
        Run: jira auth login --user {id} (the person must approve the login in a browser)"
    )]
    UserLoginExpired { reason: String, id: String },

    #[error("OAuth login failed: {reason}")]
    LoginFailed { reason: String },

    #[error(
        "remote login failed: {reason}. Start a new remote login with: jira auth login --user {id} \
        --remote --redirect-uri <redirect-uri>"
    )]
    RemoteLoginFailed { reason: String, id: String },

    #[error(
        "a remote login (--remote, --code, --state) logs in a person and needs --user <USER_ID>. \
        Retry with: jira auth login --user <USER_ID> --remote --redirect-uri <redirect-uri>, \
        then jira auth login --user <USER_ID> --code <CODE> --state <STATE>"
    )]
    RemoteLoginNeedsUser,

    #[error(
        "failed to save credentials to {path}: {reason}. \
        Check that the directory exists and is writable."
    )]
    SaveCredentialsFailed { path: String, reason: String },

    #[error("Jira API request failed: {reason}")]
    ApiRequestFailed { reason: String },

    #[error("Jira API returned status {status}: {body}")]
    ApiError { status: u16, body: String },

    #[error("failed to serialize response to JSON: {reason}")]
    JsonSerialize { reason: String },

    #[error(transparent)]
    Select(#[from] cli_fields::RenderError),

    #[error(
        "transition \"{name}\" not found for this issue in its current state. \
        Available transitions: {available}"
    )]
    TransitionNotFound { name: String, available: String },

    #[error(
        "deleting {key} is permanent and cannot be undone. \
        Pass --confirm to execute: jira issue delete {key} --confirm"
    )]
    DeleteNotConfirmed { key: String },

    #[error(
        "issue assign requires exactly one of --assignee or --unassign. \
        Retry with either: jira issue assign {key} --assignee <ACCOUNT_ID>  \
        or: jira issue assign {key} --unassign"
    )]
    AssignMissingTarget { key: String },

    #[error("one or more doctor checks failed. See JSON output above for details.")]
    DoctorCheckFailed,

    #[error("I/O error: {reason}")]
    IoError { reason: String },

    #[error("{field} is empty: nothing was written. Run the same jira init command again and type it, or pass it with {flag}")]
    EmptyInput { field: &'static str, flag: &'static str },

    #[error("nothing to log out: identity \"{label}\" has no stored login on this machine. To log in: {login}")]
    NothingToLogOut { label: String, login: String },
}

impl CliError {
    /// The process exit code: `oauth_user_login::NOT_LOGGED_IN_EXIT_CODE` (3)
    /// when the selected identity needs a new login (issue #194), 1 otherwise.
    pub fn exit_code(&self) -> u8 {
        match self {
            CliError::NotAuthenticatedService | CliError::NotAuthenticatedUser { .. } | CliError::UserLoginExpired { .. } => {
                oauth_user_login::NOT_LOGGED_IN_EXIT_CODE
            }
            _ => 1,
        }
    }
}
