//! Top-level error type for the `google-chat` CLI.
//!
//! `CliError` is the single error type that surfaces to the user. Every
//! variant carries a self-contained message — what went wrong and what the
//! caller (human or LLM) should do to fix it. Messages are plain text with
//! no colors, symbols, or formatting.
//!
//! Internal module errors (`LoginError`, `OAuthConfigError`) are mapped to
//! `CliError` at the `run()` boundary in `main.rs` or in the relevant
//! command handler. They are never exposed directly to the user.

use thiserror::Error;

/// Top-level CLI error. Every variant carries a self-contained message:
/// what went wrong and what the caller should do to fix it.
/// No colors, symbols, or formatting — output is designed to be read by an LLM.
#[derive(Debug, Error)]
pub enum CliError {
    #[error(
        "app credentials file not found at {path}. \
        Create it manually with your Google OAuth 2.0 Desktop app credentials: \
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

    #[error("OAuth login failed: {reason}")]
    LoginFailed { reason: String },

    #[error(
        "failed to save credentials to {path}: {reason}. \
        Check that the directory exists and is writable."
    )]
    SaveCredentialsFailed { path: String, reason: String },

    #[error(
        "not authenticated. \
        Run: google-chat auth login"
    )]
    NotAuthenticated,

    #[error(
        "failed to refresh authentication token: {reason}. \
        The session may have been revoked. Run: google-chat auth login"
    )]
    TokenRefreshFailed { reason: String },

    #[error("failed to serialize response to JSON: {reason}")]
    JsonSerialize { reason: String },

    #[error("one or more doctor checks failed. See JSON output above for details.")]
    DoctorCheckFailed,
}
