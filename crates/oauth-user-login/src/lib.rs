//! Provider-agnostic plumbing for an OAuth 2.0 authorization-code login run by
//! a human (`auth login --user` in every CLI of this workspace): PKCE, the CSRF
//! `state`, and the loopback listener that receives the browser's redirect.
//!
//! Everything that varies by provider stays in each crate: the authorize URL's
//! parameters, the token endpoint and its auth style, scopes, and refresh
//! semantics. Errors here are generic; each crate wraps them in its own
//! `LoginError` so the message carries that CLI's exact retry command.

mod callback;
mod pkce;

pub use callback::{
    CallbackError, CallbackParams, ListenerError, WaitError, bind_listener,
    parse_callback_request_line, wait_for_callback,
};
pub use pkce::{code_challenge, generate_code_verifier, generate_state};
