//! Top-level error type for the `bitbucket` CLI.
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

#[derive(Debug, Error)]
pub enum CliError {
    #[error(
        "app credentials file not found at {path}. Create a Bitbucket OAuth consumer \
        (workspace Settings -> OAuth consumers) and run: \
        bitbucket init --client-id <KEY> --client-secret <SECRET> (the app identity, the default) \
        and/or bitbucket init --user <USER_ID> --client-id <KEY> --client-secret <SECRET> (a person, used with --user <USER_ID>)"
    )]
    AppConfigNotFound { path: String },

    #[error(
        "app credentials file at {path} is not valid: {reason}. Expected format: \
        {{\"service\": {{\"client_id\": \"...\", \"client_secret\": \"...\"}}, \
        \"user\": {{\"client_id\": \"...\", \"client_secret\": \"...\"}}}} (either section may be omitted). \
        Fix or delete the file, then run bitbucket init"
    )]
    AppConfigInvalid { path: String, reason: String },

    #[error(
        "app.json at {path} uses the old single-identity format (client_id at top level). \
        Recreate it: bitbucket init --client-id <KEY> --client-secret <SECRET> for the app identity, \
        and bitbucket init --user <USER_ID> --client-id <KEY> --client-secret <SECRET> for the people used with --user \
        (the same consumer can serve both)"
    )]
    AppConfigLegacy { path: String },

    #[error(
        "app.json at {path} has no \"service\" section (the OAuth consumer used without --user, acting as \
        the app). Run: bitbucket init --client-id <KEY> --client-secret <SECRET>"
    )]
    ServiceAppMissing { path: String },

    #[error(
        "app.json at {path} has no \"user\" section (the OAuth consumer used with --user to act as a person, \
        every person through it; it may be the same consumer). \
        Run: bitbucket init --user {id} --client-id <KEY> --client-secret <SECRET>, \
        or bitbucket init --user-app --client-id <KEY> --client-secret <SECRET> to set it up without logging anyone in"
    )]
    UserAppMissing { path: String, id: String },

    #[error(
        "init --user-app sets up the OAuth consumer every person logs in through and logs nobody in, so it takes no --user. \
        Run: bitbucket init --user-app --client-id <KEY> --client-secret <SECRET>, then bitbucket auth login --user {id}; \
        or bitbucket init --user {id} --client-id <KEY> --client-secret <SECRET> to do both at once"
    )]
    UserAppWithUser { id: String },

    #[error(
        "no home directory found — cannot resolve config path. \
        Set the XDG_CONFIG_HOME environment variable explicitly."
    )]
    NoHomeDirectory,

    #[error(
        "not logged in as the OAuth app. Run: bitbucket auth login. \
        To act as a person logged in with bitbucket auth login --user <USER_ID>, pass --user <USER_ID> instead"
    )]
    NotAuthenticatedService,

    #[error(
        "user {id} is not logged in. Run: bitbucket auth login --user {id} \
        (the person must approve the login in a browser)"
    )]
    NotAuthenticatedUser { id: String },

    #[error(
        "failed to renew the OAuth app's token: {reason}. Check that the consumer Key/Secret in \
        app.json's \"service\" section are still valid, then run: bitbucket auth login"
    )]
    TokenRenewalFailedService { reason: String },

    #[error(
        "failed to refresh the token of user {id}: {reason}. The refresh token may have expired \
        (unused for 3 months) or been revoked. Run: bitbucket auth login --user {id}"
    )]
    TokenRefreshFailedUser { reason: String, id: String },

    #[error(
        "the login of user {id} is no longer valid ({reason}): the refresh token expired (unused for 3 months) \
        or was revoked. Run: bitbucket auth login --user {id} (the person must approve the login in a browser)"
    )]
    UserLoginExpired { reason: String, id: String },

    #[error("OAuth login failed: {reason}")]
    LoginFailed { reason: String },

    #[error("remote login failed: {reason}. Start a new remote login with: bitbucket auth login --user {id} --remote")]
    RemoteLoginFailed { reason: String, id: String },

    #[error(
        "a remote login (--remote, --code, --state) logs in a person and needs --user <USER_ID>. \
        Retry with: bitbucket auth login --user <USER_ID> --remote, \
        then bitbucket auth login --user <USER_ID> --code <CODE> --state <STATE>"
    )]
    RemoteLoginNeedsUser,

    #[error(
        "failed to save credentials to {path}: {reason}. \
        Check that the directory exists and is writable."
    )]
    SaveCredentialsFailed { path: String, reason: String },

    #[error("Bitbucket API request failed: {reason}")]
    ApiRequestFailed { reason: String },

    #[error(
        "invalid repository identifier '{value}'. \
        Expected the form workspace/repo_slug, e.g. lucabrognaracode/my-repo"
    )]
    InvalidRepository { value: String },

    #[error("failed to serialize response to JSON: {reason}")]
    JsonSerialize { reason: String },

    #[error(transparent)]
    Select(#[from] cli_fields::RenderError),

    #[error("doctor check failed. See the report above for details.")]
    DoctorCheckFailed,

    #[error("I/O error: {reason}")]
    IoError { reason: String },

    #[error("{field} is empty: nothing was written. Run bitbucket init again and type it, or pass it with {flag}")]
    EmptyInput { field: &'static str, flag: &'static str },

    #[error("nothing to log out: identity \"{label}\" has no stored login on this machine. To log in: {login}")]
    NothingToLogOut { label: String, login: String },

    #[error("invalid input: {reason}")]
    InvalidInput { reason: String },

    #[error(
        "declining pull request {id} changes its state and cannot be undone by this CLI. \
        Pass --confirm to execute: bitbucket pr decline {repository} {id} --confirm"
    )]
    DeclineNotConfirmed { repository: String, id: u64 },

    #[error(
        "merging pull request {id} is permanent and cannot be undone. \
        Pass --confirm to execute: bitbucket pr merge {repository} {id} --confirm"
    )]
    MergeNotConfirmed { repository: String, id: u64 },

    #[error(
        "deleting repository {repository} is permanent and cannot be undone. \
        Pass --confirm to execute: bitbucket repo delete {repository} --confirm"
    )]
    RepoDeleteNotConfirmed { repository: String },
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
