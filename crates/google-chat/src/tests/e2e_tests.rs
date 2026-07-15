//! End-to-end tests against a real Google Chat account.
//!
//! # Prerequisites
//!
//! - `google-chat auth login --user` (or `init`) must have been run on this
//!   machine — these tests use whatever credentials are already stored.
//! - Some tests additionally need `GOOGLE_CHAT_E2E_SPACE` set to a Chat
//!   space safe for repeated automated checks — either exported inline per
//!   run, or via a workspace-root `.env` file (see `.env.example`; loaded
//!   automatically by `setup()` below, an already-exported value always
//!   wins over `.env`).
//!
//! # Running
//!
//! ```sh
//! cargo test -p google-chat -- --ignored
//! ```
//!
//! # Scope: read-only by design
//!
//! Unlike jira's e2e suite, these tests are deliberately read-only —
//! `spaces.list` and `messages.list` only. `messages send` creates real,
//! visible messages in spaces shared with real people (colleagues), and
//! `messages delete` permanently removes real messages, so neither is
//! covered by an automated/repeatable e2e test — see `BACKLOG.md` GCHAT-2
//! for the reasoning. Manual live verification (as done while implementing
//! each command) is the only check for both.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::auth;
use crate::client::GoogleChatClient;
use crate::context;

/// Builds an authenticated `GoogleChatClient`. Loads `.env` from the
/// workspace root first (if present) so `GOOGLE_CHAT_E2E_SPACE` doesn't need
/// to be exported inline every run — an already-exported value still takes
/// precedence.
fn setup() -> GoogleChatClient {
    dotenvy::dotenv().ok();
    let config_dir = context::config_dir().expect("could not resolve config dir");
    let oauth_config = auth::OAuthConfig::load(&auth::app_config_path(&config_dir))
        .expect("app.json not found — run `google-chat init` first");
    let credentials =
        auth::load_credentials(&oauth_config, &auth::credentials_path(&config_dir))
            .expect("not authenticated — run `google-chat auth login --user` first");
    GoogleChatClient::new(&credentials)
}

/// Returns the designated e2e test space from the environment, panicking
/// with a clear message if unset.
fn test_space() -> String {
    std::env::var("GOOGLE_CHAT_E2E_SPACE")
        .expect("set GOOGLE_CHAT_E2E_SPACE to a Chat space safe for automated e2e checks")
}

#[test]
#[ignore = "e2e: requires credentials"]
fn e2e_spaces_list_returns_at_least_one_space() {
    let client = setup();

    let response = client.list_spaces(10, None).expect("spaces.list should succeed");

    let spaces = response["spaces"]
        .as_array()
        .expect("response must contain a spaces array");
    assert!(
        !spaces.is_empty(),
        "expected at least one space for this account — if this account genuinely has \
         none, this assertion needs relaxing, but that would also make most of this \
         crate untestable"
    );
    assert!(
        spaces[0]["name"].as_str().is_some_and(|n| n.starts_with("spaces/")),
        "each space must have a name field shaped like \"spaces/{{id}}\""
    );
}

#[test]
#[ignore = "e2e: requires credentials"]
fn e2e_messages_list_on_first_space_succeeds() {
    let client = setup();

    let spaces_response = client.list_spaces(1, None).expect("spaces.list should succeed");
    let first_space = spaces_response["spaces"][0]["name"]
        .as_str()
        .expect("expected at least one space with a name field");

    // Read-only smoke check: the call must succeed and return a well-formed
    // response, whether or not the space happens to have any messages. A
    // missing/non-array "messages" field would mean the response shape
    // changed; an empty array is a perfectly valid result and not a failure.
    let response = client
        .list_messages(first_space, 10, None, None)
        .unwrap_or_else(|e| panic!("messages.list should succeed for {first_space}: {e}"));

    assert!(
        response.get("messages").is_none_or(serde_json::Value::is_array),
        "if present, \"messages\" must be an array"
    );
}

#[test]
#[ignore = "e2e: requires credentials and GOOGLE_CHAT_E2E_SPACE"]
fn e2e_messages_list_on_designated_test_space_succeeds() {
    let client = setup();
    let space = test_space();

    // Unlike e2e_messages_list_on_first_space_succeeds (whatever space
    // happens to be first for this account), this targets the specific
    // space designated safe for repeated automated checks — the
    // prerequisite BACKLOG.md's GCHAT-2 needs before messages send/delete
    // can get their own automated e2e coverage.
    let response = client
        .list_messages(&space, 10, None, None)
        .unwrap_or_else(|e| panic!("messages.list should succeed for {space}: {e}"));

    assert!(
        response.get("messages").is_none_or(serde_json::Value::is_array),
        "if present, \"messages\" must be an array"
    );
}
