//! The access token an API client sends, renewed once when the API answers
//! 401 (issue #240): a token can be revoked before it expires (the person ended
//! their session, revoked the app, a rotated refresh token), and only the
//! expiry time was checked before sending it. Transport-agnostic: each crate's
//! client passes the closure that sends its request and says what a 401 is.

use std::cell::RefCell;

/// Renews the token the API answered 401 to (the argument) and returns the new
/// one. The error is the crate's own, ready for the user (a person whose
/// refresh token was refused gets exit code 3).
pub type Renewer<E> = Box<dyn Fn(&str) -> Result<String, E>>;

pub struct BearerToken<E> {
    token: RefCell<String>,
    renewer: Option<Renewer<E>>,
}

impl<E> BearerToken<E> {
    /// A token that is sent as is, never renewed.
    pub fn new(token: String) -> Self {
        Self { token: RefCell::new(token), renewer: None }
    }

    /// Renews the token through `renewer` when a call is answered 401.
    #[must_use]
    pub fn with_renewer(mut self, renewer: Renewer<E>) -> Self {
        self.renewer = Some(renewer);
        self
    }

    /// The token the next call will send.
    pub fn current(&self) -> String {
        self.token.borrow().clone()
    }

    /// Calls `send` with the current token. If `unauthorized` says the response
    /// is a 401 and a renewer is set, renews the token once and calls `send`
    /// again with the new one; that second response is returned whatever it
    /// is. A 401 means the request was not processed, so repeating it is safe.
    pub fn send<T>(&self, send: impl Fn(&str) -> T, unauthorized: impl Fn(&T) -> bool) -> Result<T, E> {
        let response = send(&self.current());
        let Some(renewer) = &self.renewer else {
            return Ok(response);
        };
        if !unauthorized(&response) {
            return Ok(response);
        }
        let fresh = renewer(&self.current())?;
        *self.token.borrow_mut() = fresh;
        Ok(send(&self.current()))
    }
}

#[cfg(test)]
#[path = "tests/bearer_tests.rs"]
mod tests;
