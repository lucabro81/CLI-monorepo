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

    #[error("not authenticated ({reason}). Run: zitadel auth login")]
    NotAuthenticated { reason: String },

    #[error(
        "failed to renew the access token: {reason}. Run: zitadel auth login \
        (if it keeps failing, check the service user key in app.json is still valid in the console)"
    )]
    TokenRefreshFailed { reason: String },

    #[error(transparent)]
    Select(#[from] cli_fields::RenderError),

    #[error("ZITADEL API request failed: {reason}. Check instance_url in app.json and network connectivity.")]
    ApiRequestFailed { reason: String },

    #[error("ZITADEL rejected the access token (401): {body}. Run: zitadel auth login")]
    ApiUnauthorized { body: String },

    #[error(
        "ZITADEL denied the operation (403): {body}. The logged-in identity lacks the \
        administrator role this operation requires (e.g. IAM_OWNER, ORG_OWNER, ORG_USER_MANAGER). \
        Run: zitadel doctor to see its current roles, then grant the missing one in the console."
    )]
    ApiForbidden { body: String },

    #[error("ZITADEL API returned status {status}: {body}")]
    ApiError { status: u16, body: String },
}
