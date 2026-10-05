//! Provider-agnostic plumbing for an OAuth 2.0 authorization-code login run by
//! a human (`auth login --user` in every CLI of this workspace): PKCE, the CSRF
//! `state`, and the loopback listener that receives the browser's redirect.
//! For a login where the person is not at the CLI's machine
//! (`auth login --user --remote`), [`PendingLogin`] keeps `state`, the PKCE
//! verifier and the redirect URI on disk between the two steps.
//!
//! Everything that varies by provider stays in each crate: the authorize URL's
//! parameters, the token endpoint and its auth style, scopes, and refresh
//! semantics. Errors here are generic; each crate wraps them in its own
//! `LoginError` so the message carries that CLI's exact retry command.

mod callback;
mod pending;
mod pkce;

pub use callback::{
    CallbackError, CallbackParams, ListenerError, WaitError, bind_listener,
    parse_callback_request_line, wait_for_callback,
};
pub use pending::{
    PENDING_LOGIN_TTL_SECS, PendingLogin, PendingLoginError, PendingLoginStatus, pending_login_status,
    rfc3339_utc, take_pending_login,
};
pub use pkce::{code_challenge, generate_code_verifier, generate_state};
