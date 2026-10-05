#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::thread;

use super::{
    CallbackError, CallbackParams, WaitError, bind_listener, parse_callback_request_line,
    wait_for_callback,
};

fn params(code: &str, state: &str) -> CallbackParams {
    CallbackParams { code: code.to_string(), state: state.to_string() }
}

// ── parse_callback_request_line ───────────────────────────────────────────

#[test]
fn parses_code_and_state() {
    assert_eq!(
        parse_callback_request_line("GET /callback?code=abc&state=xyz HTTP/1.1", "/callback"),
        Ok(params("abc", "xyz"))
    );
}

#[test]
fn decodes_url_encoded_values() {
    assert_eq!(
        parse_callback_request_line("GET /callback?code=abc%2F1%2B2%3D&state=x%20y HTTP/1.1", "/callback"),
        Ok(params("abc/1+2=", "x y"))
    );
}

#[test]
fn ignores_extra_query_params() {
    assert_eq!(
        parse_callback_request_line(
            "GET /callback?scope=a+b&code=abc&authuser=0&state=xyz&prompt=consent HTTP/1.1",
            "/callback"
        ),
        Ok(params("abc", "xyz"))
    );
}

#[test]
fn honours_the_given_callback_path() {
    assert_eq!(
        parse_callback_request_line("GET /oauth/cb?code=abc&state=xyz HTTP/1.1", "/oauth/cb"),
        Ok(params("abc", "xyz"))
    );
}

// Regression: a provider redirecting to a registered URL whose path differs
// from the one the CLI listens on (Bitbucket sends no redirect_uri, so the
// consumer's callback URL wins) got a 404 and the login waited forever.
#[test]
fn an_oauth_redirect_on_another_path_names_the_expected_path() {
    for (line, path) in [
        ("GET /?code=abc&state=xyz HTTP/1.1", "/"),
        ("GET /oauth/cb?state=xyz HTTP/1.1", "/oauth/cb"),
        ("GET /cb?error=access_denied HTTP/1.1", "/cb"),
    ] {
        assert_eq!(
            parse_callback_request_line(line, "/callback"),
            Err(CallbackError::WrongPath { path: path.to_string(), expected: "/callback".to_string() }),
            "line {line:?}"
        );
    }
}

#[test]
fn wrong_path_message_says_what_to_register() {
    let err = CallbackError::WrongPath { path: "/".to_string(), expected: "/callback".to_string() };

    assert_eq!(
        err.to_string(),
        "the provider redirected to path \"/\", but this CLI listens on \"/callback\". Change the \
        redirect (callback) URL registered with the provider so its path is exactly \"/callback\", \
        then retry the login"
    );
}

#[test]
fn error_param_is_a_denial_with_description() {
    assert_eq!(
        parse_callback_request_line(
            "GET /callback?error=access_denied&error_description=user+cancelled&state=xyz HTTP/1.1",
            "/callback"
        ),
        Err(CallbackError::Denied {
            error: "access_denied".to_string(),
            description: Some("user cancelled".to_string()),
        })
    );
}

#[test]
fn error_param_takes_precedence_over_code() {
    assert_eq!(
        parse_callback_request_line("GET /callback?code=abc&error=access_denied&state=xyz HTTP/1.1", "/callback"),
        Err(CallbackError::Denied { error: "access_denied".to_string(), description: None })
    );
}

#[test]
fn missing_code_or_state_is_reported_by_name() {
    assert_eq!(
        parse_callback_request_line("GET /callback?state=xyz HTTP/1.1", "/callback"),
        Err(CallbackError::MissingParam("code"))
    );
    assert_eq!(
        parse_callback_request_line("GET /callback?code=abc HTTP/1.1", "/callback"),
        Err(CallbackError::MissingParam("state"))
    );
    // The callback path with no query at all: still the callback, just empty.
    assert_eq!(
        parse_callback_request_line("GET /callback HTTP/1.1", "/callback"),
        Err(CallbackError::MissingParam("code"))
    );
    assert_eq!(
        parse_callback_request_line("GET /callback? HTTP/1.1", "/callback"),
        Err(CallbackError::MissingParam("code"))
    );
}

#[test]
fn anything_else_is_not_the_callback() {
    for line in [
        "GET /favicon.ico HTTP/1.1",
        "GET /favicon.ico?v=2 HTTP/1.1",
        "POST /callback?code=abc&state=xyz HTTP/1.1",
        "GET /callback?code=abc&state=xyz",
        "garbage",
        "",
    ] {
        assert_eq!(
            parse_callback_request_line(line, "/callback"),
            Err(CallbackError::NotCallback),
            "line {line:?}"
        );
    }
}

#[test]
fn denial_message_is_actionable_with_and_without_description() {
    let with = CallbackError::Denied {
        error: "access_denied".to_string(),
        description: Some("user cancelled".to_string()),
    };
    let without = CallbackError::Denied { error: "access_denied".to_string(), description: None };

    assert_eq!(
        with.to_string(),
        "authorization denied: access_denied (user cancelled). Approve the consent page to log in, \
        then retry the login"
    );
    assert_eq!(
        without.to_string(),
        "authorization denied: access_denied. Approve the consent page to log in, then retry the login"
    );
}

// ── bind_listener ─────────────────────────────────────────────────────────

#[test]
fn bind_listener_binds_a_free_address() {
    let listener = bind_listener("127.0.0.1:0").unwrap();

    assert!(listener.local_addr().unwrap().ip().is_loopback());
}

#[test]
fn bind_listener_on_a_busy_port_names_the_port_to_free() {
    let busy = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = busy.local_addr().unwrap().to_string();
    let port = busy.local_addr().unwrap().port().to_string();

    let err = bind_listener(&address).unwrap_err();

    assert_eq!(err.address, address);
    assert_eq!(err.port, port);
    assert!(
        err.to_string().starts_with(&format!("cannot listen for the login callback on {address}: ")),
        "got {err}"
    );
    assert!(err.to_string().ends_with(&format!("(find it with `lsof -i :{port}`) and retry")), "got {err}");
}

// ── wait_for_callback ─────────────────────────────────────────────────────

fn send(addr: SocketAddr, request_line: &str) -> String {
    let mut stream = TcpStream::connect(addr).unwrap();
    stream
        .write_all(format!("{request_line}\r\nHost: localhost\r\n\r\n").as_bytes())
        .unwrap();
    let mut response = String::new();
    stream.read_to_string(&mut response).unwrap();
    response
}

/// Runs `wait_for_callback` while a "browser" thread sends `requests` in order,
/// returning the outcome and every response the browser received.
fn run(requests: &'static [&'static str], expected_state: &str) -> (Result<CallbackParams, WaitError>, Vec<String>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let browser = thread::spawn(move || requests.iter().map(|r| send(addr, r)).collect::<Vec<_>>());

    let outcome = wait_for_callback(&listener, "/callback", expected_state);
    (outcome, browser.join().unwrap())
}

// Regression: the atlassian-auth and google-chat copies accepted exactly one
// connection, so a browser's /favicon.ico probe arriving first ended the login
// with a parse error (issue #143).
#[test]
fn stray_requests_get_404_and_the_listener_keeps_waiting() {
    let (outcome, responses) = run(
        &["GET /favicon.ico HTTP/1.1", "GET /robots.txt HTTP/1.1", "GET /callback?code=c1&state=s1 HTTP/1.1"],
        "s1",
    );

    assert_eq!(outcome.unwrap(), params("c1", "s1"));
    assert!(responses[0].starts_with("HTTP/1.1 404 Not Found\r\n"), "got {}", responses[0]);
    assert!(responses[1].starts_with("HTTP/1.1 404 Not Found\r\n"), "got {}", responses[1]);
    assert!(responses[2].starts_with("HTTP/1.1 200 OK\r\n"), "got {}", responses[2]);
    assert!(responses[2].ends_with("Login complete. You can close this window."), "got {}", responses[2]);
}

#[test]
fn a_state_mismatch_aborts_with_400() {
    let (outcome, responses) = run(&["GET /callback?code=c1&state=forged HTTP/1.1"], "expected");

    assert!(matches!(outcome, Err(WaitError::StateMismatch)), "got {outcome:?}");
    assert!(responses[0].starts_with("HTTP/1.1 400 Bad Request\r\n"), "got {}", responses[0]);
    assert!(responses[0].ends_with("Login failed. See the CLI output for details."), "got {}", responses[0]);
}

// Regression: the atlassian-auth and google-chat copies ignored the `error`
// parameter and reported a missing `code` instead of the denial (issue #70,
// found again while extracting this library in #143).
#[test]
fn a_denied_consent_is_reported_as_such() {
    let (outcome, responses) = run(
        &["GET /callback?error=access_denied&error_description=nope&state=s1 HTTP/1.1"],
        "s1",
    );

    match outcome {
        Err(WaitError::Callback(CallbackError::Denied { error, description })) => {
            assert_eq!(error, "access_denied");
            assert_eq!(description.as_deref(), Some("nope"));
        }
        other => panic!("expected a denial, got {other:?}"),
    }
    assert!(responses[0].starts_with("HTTP/1.1 400 Bad Request\r\n"), "got {}", responses[0]);
}

#[test]
fn an_empty_connection_gets_404_and_the_listener_keeps_waiting() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let browser = thread::spawn(move || {
        // Connects and closes without sending a request line (e.g. a probe).
        let mut empty = TcpStream::connect(addr).unwrap();
        empty.shutdown(std::net::Shutdown::Write).unwrap();
        let mut response = String::new();
        empty.read_to_string(&mut response).unwrap();
        (response, send(addr, "GET /callback?code=c1&state=s1 HTTP/1.1"))
    });

    let outcome = wait_for_callback(&listener, "/callback", "s1");
    let (empty, callback) = browser.join().unwrap();

    assert_eq!(outcome.unwrap(), params("c1", "s1"));
    assert_eq!(
        empty,
        "HTTP/1.1 404 Not Found\r\nContent-Type: text/plain; charset=utf-8\r\nContent-Length: 10\r\nConnection: close\r\n\r\nNot found."
    );
    assert!(callback.starts_with("HTTP/1.1 200 OK\r\n"), "got {callback}");
}

#[test]
fn a_callback_without_state_aborts_with_400() {
    let (outcome, responses) = run(&["GET /callback?code=c1 HTTP/1.1"], "s1");

    assert!(
        matches!(outcome, Err(WaitError::Callback(CallbackError::MissingParam("state")))),
        "got {outcome:?}"
    );
    assert!(responses[0].starts_with("HTTP/1.1 400 Bad Request\r\n"), "got {}", responses[0]);
}

#[test]
fn a_callback_without_code_aborts_with_400() {
    let (outcome, responses) = run(&["GET /callback?state=s1 HTTP/1.1"], "s1");

    assert!(
        matches!(outcome, Err(WaitError::Callback(CallbackError::MissingParam("code")))),
        "got {outcome:?}"
    );
    assert!(responses[0].starts_with("HTTP/1.1 400 Bad Request\r\n"), "got {}", responses[0]);
}

// Every message a CLI can surface ends with a retry instruction, so each crate
// can append its own command uniformly (e.g. "...: zitadel auth login --user").
#[test]
fn wait_error_messages_are_actionable() {
    assert_eq!(
        WaitError::Io(std::io::Error::other("boom")).to_string(),
        "I/O error while waiting for the login callback (boom). Retry the login"
    );
    assert_eq!(
        WaitError::StateMismatch.to_string(),
        "the login callback's state did not match the one sent (possible CSRF, or a stale browser \
        tab from an earlier attempt). Login aborted: retry it"
    );
    assert_eq!(
        WaitError::Callback(CallbackError::MissingParam("code")).to_string(),
        CallbackError::MissingParam("code").to_string()
    );
}
