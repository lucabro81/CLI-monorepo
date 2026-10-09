//! Provider-agnostic plumbing for an OAuth 2.0 authorization-code login run by
//! a human (`auth login --user` in every CLI of this workspace): PKCE, the CSRF
//! `state`, and the loopback listener that receives the browser's redirect.
//! For a login where the person is not at the CLI's machine
//! (`auth login --user --remote`), [`PendingLogin`] keeps `state`, the PKCE
//! verifier and the redirect URI on disk between the two steps. [`Identity`]
//! names the identities each CLI stores side by side (the service identity and
//! any number of humans, each named by a [`UserId`]) and their credentials
//! files; [`write_secret_file`] and [`lock_exclusive`] write those files
//! owner-only and serialize their renewal across processes.
//!
//! Everything that varies by provider stays in each crate: the authorize URL's
//! parameters, the token endpoint and its auth style, scopes, and refresh
//! semantics. Errors here are generic; each crate wraps them in its own
//! `LoginError` so the message carries that CLI's exact retry command.

mod bearer;
mod callback;
mod identity;
mod pending;
mod pkce;
mod secret_file;

pub use bearer::{BearerToken, Renewer};
pub use callback::{
    CallbackError, CallbackParams, ListenerError, WaitError, bind_listener,
    parse_callback_request_line, wait_for_callback,
};
pub use identity::{
    Identity, NOT_LOGGED_IN_EXIT_CODE, UserId, legacy_credentials_files, list_users, pending_login_path,
    remove_identity, token_request_rejected,
};
pub use pending::{
    PENDING_LOGIN_TTL_SECS, PendingLogin, PendingLoginError, PendingLoginStatus, pending_login_status,
    rfc3339_utc, take_pending_login,
};
pub use pkce::{code_challenge, generate_code_verifier, generate_state};
pub use secret_file::{FileLock, lock_exclusive, write_secret_file};
