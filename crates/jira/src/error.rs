use thiserror::Error;

/// Top-level CLI error. Every variant carries a self-contained message:
/// what went wrong and what the caller should do to fix it.
/// No colors, symbols, or formatting — output is designed to be read by an LLM.
#[derive(Debug, Error)]
pub enum CliError {
    #[error(
        "app credentials file not found at {path}. \
        Create it manually with your Atlassian OAuth 2.0 app credentials: \
        {{\"client_id\": \"...\", \"client_secret\": \"...\"}}"
    )]
    AppConfigNotFound { path: String },

    #[error(
        "app credentials file at {path} is not valid JSON: {reason}. \
        Expected format: {{\"client_id\": \"...\", \"client_secret\": \"...\"}}"
    )]
    AppConfigInvalid { path: String, reason: String },

    #[error(
        "no home directory found — cannot resolve config path. \
        Set the XDG_CONFIG_HOME environment variable explicitly."
    )]
    NoHomeDirectory,

    #[error(
        "not authenticated. \
        Run: jira auth login"
    )]
    NotAuthenticated,

    #[error(
        "failed to refresh authentication token: {reason}. \
        The session may have been revoked. Run: jira auth login"
    )]
    TokenRefreshFailed { reason: String },

    #[error("OAuth login failed: {reason}")]
    LoginFailed { reason: String },

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

    #[error("one or more doctor checks failed. See JSON output above for details.")]
    DoctorCheckFailed,

    #[error("I/O error: {reason}")]
    IoError { reason: String },
}
