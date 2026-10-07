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

    #[error(
        "login failed: {reason}. Check that instance_url in app.json is right and that the service \
        user key still exists in the console (Users > Service Users > <user> > Keys); if it was \
        deleted or expired, create a new JSON key and run: zitadel init --key-file <path-to-key.json>"
    )]
    LoginFailed { reason: String },

    #[error(
        "login failed: {reason}. Check the Native application in the console: its client id must \
        match app.json (zitadel init --client-id <client-id>), its redirect URI must be \
        http://localhost:8080/callback, authentication method PKCE, refresh token enabled. Then \
        retry: zitadel auth login --user {id}"
    )]
    UserLoginFailed { reason: String, id: String },

    #[error(
        "remote login failed: {reason}. Start a new remote login with: zitadel auth login --user {id} \
        --remote --redirect-uri <redirect-uri>"
    )]
    RemoteLoginFailed { reason: String, id: String },

    #[error(
        "failed to save credentials to {path}: {reason}. \
        Check that the directory exists and is writable."
    )]
    SaveCredentialsFailed { path: String, reason: String },

    #[error(
        "not logged in as the service user ({reason}). Run: zitadel auth login. \
        To act as a person logged in with zitadel auth login --user <USER_ID>, pass --user <USER_ID> instead"
    )]
    NotAuthenticatedService { reason: String },

    #[error(
        "user {id} is not logged in ({reason}). \
        Run: zitadel auth login --user {id} (the person must approve the login in a browser)"
    )]
    NotAuthenticatedUser { reason: String, id: String },

    #[error(
        "failed to renew the access token: {reason}. Run: zitadel auth login \
        (if it keeps failing, check the service user key in app.json is still valid in the console)"
    )]
    TokenRenewalFailedService { reason: String },

    #[error(
        "failed to refresh the token of user {id}: {reason}. \
        The refresh token may have expired or been revoked. Run: zitadel auth login --user {id}"
    )]
    TokenRefreshFailedUser { reason: String, id: String },

    #[error(
        "the login of user {id} is no longer valid ({reason}): the refresh token expired or was revoked. \
        Run: zitadel auth login --user {id} (the person must approve the login in a browser)"
    )]
    UserLoginExpired { reason: String, id: String },

    #[error(
        "a remote login (--remote, --code, --state) logs in a person and needs --user <USER_ID>. \
        Retry with: zitadel auth login --user <USER_ID> --remote --redirect-uri <redirect-uri>, \
        then zitadel auth login --user <USER_ID> --code <CODE> --state <STATE>"
    )]
    RemoteLoginNeedsUser,

    #[error(transparent)]
    Select(#[from] cli_fields::RenderError),

    #[error("ZITADEL API request failed: {reason}. Check instance_url in app.json and network connectivity.")]
    ApiRequestFailed { reason: String },

    #[error(
        "ZITADEL rejected the access token (401): {body}. Run: zitadel auth login \
        (zitadel auth login --user <USER_ID> if the command was run with --user <USER_ID>)"
    )]
    ApiUnauthorized { body: String },

    #[error(
        "ZITADEL denied the operation (403): {body}. The logged-in identity lacks the \
        administrator role this operation requires (e.g. IAM_OWNER, ORG_OWNER, ORG_USER_MANAGER). \
        Run: zitadel doctor to see its current roles, then grant the missing one in the console."
    )]
    ApiForbidden { body: String },

    #[error(
        "ZITADEL found no such resource (404): {body}. Verify the id — find the right one with the \
        matching search/list command (e.g. zitadel user search)."
    )]
    ApiNotFound { body: String },

    #[error(
        "ZITADEL API returned status {status}: {body}. A 4xx means the request was rejected — \
        check the flags against the command's --help; a 5xx is a ZITADEL-side error — retry later."
    )]
    ApiError { status: u16, body: String },

    #[error("one or more doctor checks failed. See the JSON report above for details.")]
    DoctorCheckFailed,

    #[error("I/O error: {reason}")]
    IoError { reason: String },

    #[error(
        "init --user-app sets up the Native app every person logs in with and logs nobody in, so it takes no --user. \
        Run: zitadel init --user-app --client-id <client-id>, then zitadel auth login --user {id}; \
        or zitadel init --user {id} --client-id <client-id> to do both at once"
    )]
    UserAppWithUser { id: String },

    #[error("nothing to log out: identity \"{label}\" has no stored login on this machine. To log in: {login}")]
    NothingToLogOut { label: String, login: String },

    #[error(
        "no instance URL known. Pass it explicitly: \
        zitadel init --instance-url https://<instance>.zitadel.cloud --key-file <path-to-key.json>"
    )]
    InstanceUrlRequired,

    #[error(
        "invalid --instance-url {value:?}: expected an absolute URL like https://<instance>.zitadel.cloud \
        (or your self-hosted domain, including https://)"
    )]
    InvalidInstanceUrl { value: String },

    #[error("cannot read key file {path}: {reason}. Check the path passed to --key-file.")]
    KeyFileUnreadable { path: String, reason: String },

    #[error(
        "key file {path} is not usable: {reason}. Download a JSON key for the service user \
        from the console and pass it to --key-file."
    )]
    InvalidKeyFile { path: String, reason: String },

    #[error("failed to write app config {path}: {reason}. Check that the directory is writable.")]
    WriteAppConfigFailed { path: String, reason: String },

    /// Conditions that should be unreachable (e.g. re-parsing a config built from
    /// typed fields) — never expected to fire.
    #[error("internal error: {reason}. This is a bug in the zitadel CLI.")]
    Internal { reason: String },
}

impl CliError {
    /// The process exit code: `oauth_user_login::NOT_LOGGED_IN_EXIT_CODE` (3)
    /// when the selected identity needs a new login (issue #194), 1 otherwise.
    pub fn exit_code(&self) -> u8 {
        match self {
            CliError::NotAuthenticatedService { .. }
            | CliError::NotAuthenticatedUser { .. }
            | CliError::UserLoginExpired { .. } => oauth_user_login::NOT_LOGGED_IN_EXIT_CODE,
            _ => 1,
        }
    }
}
