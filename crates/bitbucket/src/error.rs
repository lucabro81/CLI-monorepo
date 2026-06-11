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
        "app credentials file not found at {path}. \
        Create a Bitbucket OAuth consumer (workspace Settings -> OAuth consumers, \
        no callback URL needed for client_credentials) and write its Key/Secret to \
        this file as: {{\"client_id\": \"...\", \"client_secret\": \"...\"}}"
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

    #[error("not authenticated. Run: bitbucket auth login")]
    NotAuthenticated,

    #[error(
        "failed to refresh authentication token: {reason}. \
        Check that the OAuth consumer credentials in app.json are still valid."
    )]
    TokenRefreshFailed { reason: String },

    #[error("OAuth login failed: {reason}")]
    LoginFailed { reason: String },

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
}
