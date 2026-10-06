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
        and/or jira init --user --client-id <ID> --client-secret <SECRET> (3LO app, used with --user)"
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
        and jira init --user --client-id <ID> --client-secret <SECRET> for the 3LO app used with --user"
    )]
    AppConfigLegacy { path: String },

    #[error(
        "app.json at {path} has no \"service\" section (the Service Account credential used without --user). \
        Run: jira init --client-id <ID> --client-secret <SECRET>"
    )]
    ServiceAppMissing { path: String },

    #[error(
        "app.json at {path} has no \"user\" section (the 3LO app used with --user). \
        Run: jira init --user --client-id <ID> --client-secret <SECRET>"
    )]
    UserAppMissing { path: String },

    #[error(
        "no home directory found — cannot resolve config path. \
        Set the XDG_CONFIG_HOME environment variable explicitly."
    )]
    NoHomeDirectory,

    #[error(
        "not logged in as the service account. Run: jira auth login. \
        To act as the human logged in with jira auth login --user, pass --user instead"
    )]
    NotAuthenticatedService,

    #[error(
        "not logged in as a human. Run: jira auth login --user (a person must approve the login in a browser)"
    )]
    NotAuthenticatedUser,

    #[error(
        "failed to refresh authentication token: {reason}. \
        The session may have been revoked. Run: {login}"
    )]
    TokenRefreshFailed { reason: String, login: &'static str },

    #[error("OAuth login failed: {reason}")]
    LoginFailed { reason: String },

    #[error(
        "remote login failed: {reason}. Start a new remote login with: jira auth login --user \
        --remote --redirect-uri <redirect-uri>"
    )]
    RemoteLoginFailed { reason: String },

    #[error(
        "a remote login (--remote, --code, --state) logs in the human identity and needs --user. \
        Retry with --user: jira auth login --user --remote --redirect-uri <redirect-uri>, \
        then jira auth login --user --code <CODE> --state <STATE>"
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
}
