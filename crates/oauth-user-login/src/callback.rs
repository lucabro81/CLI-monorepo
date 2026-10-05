//! The loopback side of the login: bind the redirect address, wait for the
//! browser's callback request, and extract `code`/`state` from it.

use std::net::TcpListener;

/// `code` and `state` extracted from the authorization callback.
#[derive(Debug, PartialEq, Eq)]
pub struct CallbackParams {
    pub code: String,
    pub state: String,
}

#[derive(Debug, PartialEq, Eq, thiserror::Error)]
pub enum CallbackError {
    /// Any request that isn't `GET <callback_path>?...` (e.g. the browser's
    /// `/favicon.ico` probe). The listener answers 404 and keeps waiting.
    #[error("the request is not the OAuth callback")]
    NotCallback,
    #[error(
        "the login callback has no \"{0}\" parameter. Check that the redirect URI registered \
        with the provider matches the one this CLI uses, then retry the login"
    )]
    MissingParam(&'static str),
    #[error(
        "authorization denied: {error}{}. Approve the consent page to log in, then retry the login",
        description.as_deref().map(|d| format!(" ({d})")).unwrap_or_default()
    )]
    Denied {
        error: String,
        description: Option<String>,
    },
}

#[derive(Debug, PartialEq, Eq, thiserror::Error)]
#[error(
    "cannot listen for the login callback on {address}: {reason}. Another process is probably \
    using that port, likely an aborted previous login: stop it (find it with `lsof -i :{port}`) \
    and retry"
)]
pub struct ListenerError {
    pub address: String,
    pub port: String,
    pub reason: String,
}

#[derive(Debug, thiserror::Error)]
pub enum WaitError {
    #[error("I/O error while waiting for the login callback ({0}). Retry the login")]
    Io(#[from] std::io::Error),
    #[error("{0}")]
    Callback(CallbackError),
    #[error(
        "the login callback's state did not match the one sent (possible CSRF, or a stale \
        browser tab from an earlier attempt). Login aborted: retry it"
    )]
    StateMismatch,
}

/// Binds the loopback address the provider redirects to (e.g. `127.0.0.1:8080`).
/// Call it before opening the browser, so a busy port fails before the person
/// has consented.
pub fn bind_listener(address: &str) -> Result<TcpListener, ListenerError> {
    TcpListener::bind(address).map_err(|e| ListenerError {
        address: address.to_string(),
        port: address.rsplit_once(':').map_or(address, |(_, port)| port).to_string(),
        reason: e.to_string(),
    })
}

/// Parses the request line of a loopback request, e.g.
/// `GET /callback?code=X&state=Y HTTP/1.1`. Only a `GET` on `callback_path`
/// counts as the callback; an `error` parameter takes precedence over `code`.
pub fn parse_callback_request_line(
    line: &str,
    callback_path: &str,
) -> Result<CallbackParams, CallbackError> {
    let mut parts = line.split_whitespace();
    let (Some("GET"), Some(target), Some(_)) = (parts.next(), parts.next(), parts.next()) else {
        return Err(CallbackError::NotCallback);
    };
    let (path, query) = target.split_once('?').unwrap_or((target, ""));
    if path != callback_path {
        return Err(CallbackError::NotCallback);
    }
    let mut params: std::collections::HashMap<String, String> =
        serde_urlencoded::from_str(query).map_err(|_| CallbackError::NotCallback)?;

    if let Some(error) = params.remove("error") {
        return Err(CallbackError::Denied {
            error,
            description: params.remove("error_description"),
        });
    }
    Ok(CallbackParams {
        code: params.remove("code").ok_or(CallbackError::MissingParam("code"))?,
        state: params.remove("state").ok_or(CallbackError::MissingParam("state"))?,
    })
}

/// Serves `listener` until the OAuth callback on `callback_path` arrives, and
/// checks its `state`. Stray requests (e.g. `/favicon.ico`) get a 404 and the
/// listener keeps waiting; the callback itself gets a short plain-text page.
pub fn wait_for_callback(
    listener: &TcpListener,
    callback_path: &str,
    expected_state: &str,
) -> Result<CallbackParams, WaitError> {
    use std::io::{BufRead, BufReader, Write};

    const FAILED: &str = "Login failed. See the CLI output for details.";
    loop {
        let (mut stream, _) = listener.accept()?;
        let mut request_line = String::new();
        BufReader::new(&stream).read_line(&mut request_line)?;

        let (status, body, outcome) =
            match parse_callback_request_line(request_line.trim_end(), callback_path) {
                Err(CallbackError::NotCallback) => ("404 Not Found", "Not found.", None),
                Err(e) => ("400 Bad Request", FAILED, Some(Err(WaitError::Callback(e)))),
                Ok(params) if params.state != expected_state => {
                    ("400 Bad Request", FAILED, Some(Err(WaitError::StateMismatch)))
                }
                Ok(params) => (
                    "200 OK",
                    "Login complete. You can close this window.",
                    Some(Ok(params)),
                ),
            };
        let response = format!(
            "HTTP/1.1 {status}\r\nContent-Type: text/plain; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        );
        let _ = stream.write_all(response.as_bytes());
        if let Some(outcome) = outcome {
            return outcome;
        }
    }
}

#[cfg(test)]
#[path = "tests/callback_tests.rs"]
mod tests;
