//! Top-level error type for the `zitadel` CLI.
//!
//! `CliError` is the single error type that surfaces to the user. Every variant
//! carries a self-contained, plain-text message: what went wrong and what the
//! caller (human or LLM) should do to fix it. Module errors (`AppConfigError`,
//! `LoginError`) are mapped to it in `context.rs` or in command handlers.

use thiserror::Error;

#[derive(Debug, Error)]
pub enum CliError {
    #[error(
        "app config file not found at {path}. \
        Run: zitadel init --instance-url https://<instance>.zitadel.cloud --key-file <path-to-key.json>"
    )]
    AppConfigNotFound { path: String },

    #[error(
        "app config file at {path} is invalid: {reason}. \
        Re-run: zitadel init --instance-url https://<instance>.zitadel.cloud --key-file <path-to-key.json>"
    )]
    AppConfigInvalid { path: String, reason: String },

    #[error(
        "no home directory found — cannot resolve config path. \
        Set the XDG_CONFIG_HOME environment variable explicitly."
    )]
    NoHomeDirectory,

    #[error("login failed: {reason}")]
    LoginFailed { reason: String },

    #[error(
        "failed to save credentials to {path}: {reason}. \
        Check that the directory exists and is writable."
    )]
    SaveCredentialsFailed { path: String, reason: String },
}
