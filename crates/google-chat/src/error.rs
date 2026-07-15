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

    #[error(transparent)]
    Select(#[from] cli_fields::RenderError),

    #[error("Google Chat API request failed: {reason}")]
    ApiRequestFailed { reason: String },

    #[error("Google Chat API returned status {status}: {body}")]
    ApiError { status: u16, body: String },

    #[error("one or more doctor checks failed. See JSON output above for details.")]
    DoctorCheckFailed,

    #[error("I/O error: {reason}")]
    IoError { reason: String },

    #[error("Workspace Events API request failed: {reason}")]
    WorkspaceEventsRequestFailed { reason: String },

    #[error("Workspace Events API returned status {status}: {body}")]
    WorkspaceEventsApiError { status: u16, body: String },

    #[error("Pub/Sub API request failed: {reason}")]
    PubsubRequestFailed { reason: String },

    #[error("Pub/Sub API returned status {status}: {body}")]
    PubsubApiError { status: u16, body: String },

    #[error(
        "Pub/Sub subscriber failed: {reason}. \
        Run: google-chat auth login --user, then retry google-chat listen"
    )]
    PubsubSubscribeFailed { reason: String },

    #[error(
        "Pub/Sub subscription {subscription} already exists with a different configuration: {reason}. \
        Pub/Sub subscription topic and filter are immutable after creation — delete the subscription \
        (gcloud pubsub subscriptions delete <name>) or use a different --pubsub-subscription name to apply this configuration."
    )]
    PubsubSubscriptionMismatch { subscription: String, reason: String },

    #[error(
        "refusing to create subscription {pubsub_subscription} without --message-filter — an unfiltered \
        pull subscription delivers events for every space that ever gets attached to it, which can flood \
        an agent's `listen` stream with messages from conversations it isn't part of. Pass --message-filter \
        with a Pub/Sub filter expression to scope delivery (e.g. hasPrefix(attributes.ce-subject, \
        \"//chat.googleapis.com/spaces/SPACE_ID\"); combine multiple spaces with OR), or pass \
        --allow-unfiltered to explicitly confirm unfiltered delivery is intended."
    )]
    MessageFilterRequired { pubsub_subscription: String },

    #[error(
        "deleting {name} is permanent and cannot be undone. \
        Pass --confirm to execute: google-chat messages delete --name {name} --confirm"
    )]
    DeleteNotConfirmed { name: String },
}
