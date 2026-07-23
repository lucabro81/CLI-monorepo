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
use crate::events_client::EventsClient;
use crate::people_client::PeopleClient;

/// Loads `.env` from the workspace root first (if present) so
/// `GOOGLE_CHAT_E2E_SPACE` doesn't need to be exported inline every run (an
/// already-exported value still takes precedence), then loads and
/// auto-renews the credentials already stored on this machine.
fn authenticated_credentials_for_e2e() -> auth::Credentials {
    dotenvy::dotenv().ok();
    let config_dir = context::config_dir().expect("could not resolve config dir");
    let oauth_config = auth::OAuthConfig::load(&auth::app_config_path(&config_dir))
        .expect("app.json not found — run `google-chat init` first");
    auth::load_credentials(&oauth_config, &auth::credentials_path(&config_dir))
        .expect("not authenticated — run `google-chat auth login --user` first")
}

/// Builds an authenticated `GoogleChatClient`.
fn setup() -> GoogleChatClient {
    GoogleChatClient::new(&authenticated_credentials_for_e2e())
}

/// Builds an authenticated `PeopleClient`, sharing the same access token
/// `setup()` would use for `GoogleChatClient` (different scope, same identity).
fn setup_people_client() -> PeopleClient {
    PeopleClient::new(&authenticated_credentials_for_e2e().access_token)
}

/// Builds an authenticated `EventsClient`, sharing the same access token
/// `setup()` would use for `GoogleChatClient` (different scopes, same identity).
fn setup_events_client() -> EventsClient {
    EventsClient::new(&authenticated_credentials_for_e2e().access_token)
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
#[ignore = "e2e: requires credentials"]
fn e2e_subscription_list_returns_well_formed_response() {
    let client = setup_events_client();

    // Read-only smoke check: the call must succeed and return a well-formed
    // response, whether or not any subscriptions are currently registered
    // for this account (subscriptions have a ~4h TTL, so there may be none).
    let response = client
        .list_subscriptions(&["google.workspace.chat.message.v1.created".to_string()], None, 50, None)
        .expect("subscriptions.list should succeed");

    assert!(
        response.get("subscriptions").is_none_or(serde_json::Value::is_array),
        "if present, \"subscriptions\" must be an array"
    );
}

#[test]
#[ignore = "e2e: requires credentials"]
fn e2e_subscription_get_resolves_a_subscription_returned_by_list() {
    let client = setup_events_client();

    let list_response = client
        .list_subscriptions(&["google.workspace.chat.message.v1.created".to_string()], None, 1, None)
        .expect("subscriptions.list should succeed");
    let Some(name) = list_response["subscriptions"][0]["name"].as_str() else {
        // No subscriptions currently registered — nothing to fetch. Not a
        // failure: unlike spaces/messages, subscriptions are ephemeral
        // (~4h TTL unless renewed) and this account may have none live.
        return;
    };

    let get_response = client
        .get_subscription(name)
        .unwrap_or_else(|e| panic!("subscriptions.get should succeed for {name}: {e}"));

    assert_eq!(get_response["name"], name);
}

#[test]
#[ignore = "e2e: requires credentials and GOOGLE_CHAT_E2E_SPACE"]
fn e2e_users_get_resolves_sender_of_a_message_in_the_designated_test_space() {
    let chat_client = setup();
    let people_client = setup_people_client();
    let space = test_space();

    // Targets the designated test space (rather than "first space", like
    // e2e_messages_list_on_first_space_succeeds) because it needs an actual
    // message with a sender to resolve, and the designated space is the one
    // curated to have safe, repeatable content for automated checks.
    let messages_response = chat_client
        .list_messages(&space, 10, None, None)
        .unwrap_or_else(|e| panic!("messages.list should succeed for {space}: {e}"));
    let sender = messages_response["messages"]
        .as_array()
        .and_then(|messages| messages.iter().find_map(|m| m["sender"]["name"].as_str()))
        .expect(
            "expected at least one message with a sender.name field in the designated test \
             space — if it genuinely has none, send a message there first",
        );

    // Read-only smoke check: only asserts the response is well-formed, not a
    // specific display name — real account data varies over time, per this
    // crate's e2e convention.
    let profile = people_client
        .get_user(sender)
        .unwrap_or_else(|e| panic!("people.get should succeed for sender {sender}: {e}"));

    assert!(
        profile.get("names").is_some_and(serde_json::Value::is_array)
            || profile.get("resourceName").is_some(),
        "expected a well-formed People API profile (a names array or a resourceName field), got: {profile}"
    );
}

#[test]
#[ignore = "e2e: requires credentials and GOOGLE_CHAT_E2E_SPACE"]
fn e2e_spaces_members_list_resolves_a_human_member_of_the_designated_test_space() {
    let chat_client = setup();
    let people_client = setup_people_client();
    let space = test_space();

    let memberships_response = chat_client
        .list_members(&space, 10, None)
        .unwrap_or_else(|e| panic!("spaces.members.list should succeed for {space}: {e}"));
    let memberships = memberships_response["memberships"]
        .as_array()
        .expect("response must contain a memberships array");
    assert!(
        !memberships.is_empty(),
        "expected at least one member in the designated test space {space}"
    );

    let human_member = memberships
        .iter()
        .find(|m| m["member"]["type"] == "HUMAN")
        .and_then(|m| m["member"]["name"].as_str())
        .expect("expected at least one HUMAN member in the designated test space");

    // Same read-only smoke check as e2e_users_get: well-formed profile, not a
    // specific display name — real account data varies over time.
    let profile = people_client
        .get_user(human_member)
        .unwrap_or_else(|e| panic!("people.get should succeed for member {human_member}: {e}"));
    assert!(
        profile.get("names").is_some_and(serde_json::Value::is_array)
            || profile.get("resourceName").is_some(),
        "expected a well-formed People API profile (a names array or a resourceName field), got: {profile}"
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
