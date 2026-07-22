//! Top-level error type for the `atlassian-admin` CLI.
//!
//! `CliError` is the single error type that surfaces to the user. Every
//! variant carries a self-contained message — what went wrong and what the
//! caller (human or LLM) should do to fix it. Messages are plain text with
//! no colors, symbols, or formatting.
//!
//! Internal module errors (`ClientError`, `AdminConfigError`) are mapped to
//! `CliError` at the `run()` boundary in `main.rs` or in the relevant
//! command handler. They are never exposed directly to the user.

use thiserror::Error;

#[derive(Debug, Error)]
pub enum CliError {
    #[error(
        "app credentials file not found at {path}. \
        Create an Organization API key at admin.atlassian.com (Organization settings -> \
        API keys) and write it to this file as: {{\"api_key\": \"...\", \"org_id\": \"...\"}}"
    )]
    AppConfigNotFound { path: String },

    #[error(
        "app credentials file at {path} is not valid JSON: {reason}. \
        Expected format: {{\"api_key\": \"...\", \"org_id\": \"...\"}}"
    )]
    AppConfigInvalid { path: String, reason: String },

    #[error(
        "no home directory found — cannot resolve config path. \
        Set the XDG_CONFIG_HOME environment variable explicitly."
    )]
    NoHomeDirectory,

    #[error(
        "failed to save app config to {path}: {reason}. \
        Check that the directory exists and is writable."
    )]
    SaveConfigFailed { path: String, reason: String },

    #[error("Atlassian Admin API request failed: {reason}")]
    ApiRequestFailed { reason: String },

    #[error("failed to serialize response to JSON: {reason}")]
    JsonSerialize { reason: String },

    #[error(transparent)]
    Select(#[from] cli_fields::RenderError),

    #[error("doctor check failed. See the report above for details.")]
    DoctorCheckFailed,
}
