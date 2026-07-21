//! Handler for the `spaces` command group (`spaces list`, `spaces members list`).
//!
//! `spaces list` delegates to `client::GoogleChatClient` alone. `spaces members list`
//! additionally resolves each member via `people_client::PeopleClient` — see
//! `build_members_response`'s doc comment for how the two are combined. Maps any
//! `ClientError` to `CliError`, then prints the result as JSON (optionally filtered
//! via `--select`).

use crate::cli::{SpaceMembersCommand, SpacesCommand};
use crate::client::{self, ClientError};
use crate::context::{authenticated_client, authenticated_credentials, print_json};
use crate::error::CliError;
use crate::people_client::PeopleClient;

/// Dispatches a `SpacesCommand` variant to the appropriate Chat API call.
pub fn run(command: SpacesCommand, select: cli_fields::Select<'_>) -> Result<(), CliError> {
    match command {
        SpacesCommand::List { page_size, page_token } => {
            let client = authenticated_client()?;
            let value = client
                .list_spaces(page_size, page_token.as_deref())
                .map_err(client_error_to_cli)?;
            print_json(&value, select)
        }
        SpacesCommand::Members {
            command: SpaceMembersCommand::List { space, page_size, page_token },
        } => {
            let credentials = authenticated_credentials()?;
            let chat_client = client::GoogleChatClient::new(&credentials);
            let people_client = PeopleClient::new(&credentials.access_token);

            let memberships_response = chat_client
                .list_members(&space, page_size, page_token.as_deref())
                .map_err(client_error_to_cli)?;

            let value = build_members_response(&memberships_response, |name| {
                people_client.get_user(name).map_err(|e| e.to_string())
            });
            print_json(&value, select)
        }
        SpacesCommand::Create { user } => {
            let client = authenticated_client()?;
            let value = client.setup_space(&user).map_err(client_error_to_cli)?;
            print_json(&value, select.or_all())
        }
    }
}

/// Whether a Chat API membership should be resolved via the People API —
/// only `HUMAN` members correspond to a real Google account with a People
/// API profile (e.g. a chat app/bot member has no such profile).
fn member_user_name(membership: &serde_json::Value) -> Option<&str> {
    if membership["member"]["type"].as_str() != Some("HUMAN") {
        return None;
    }
    membership["member"]["name"].as_str()
}

/// Builds the `spaces members list` response: resolves each `HUMAN` member's
/// People API profile via `resolve` (`PeopleClient::get_user` in production,
/// a fake closure in tests — avoiding a mocking framework this crate doesn't
/// otherwise use), collecting members that can't be resolved (non-`HUMAN`, or
/// a failed People API call — e.g. a human in a different Workspace domain,
/// the same limitation `users get` has) into `unresolved` instead of failing
/// the whole command: one bad member shouldn't hide the rest of a space's
/// roster.
fn build_members_response(
    memberships_response: &serde_json::Value,
    resolve: impl Fn(&str) -> Result<serde_json::Value, String>,
) -> serde_json::Value {
    let mut members = Vec::new();
    let mut unresolved = Vec::new();

    let memberships = memberships_response["memberships"].as_array().cloned().unwrap_or_default();
    for membership in &memberships {
        if let Some(name) = member_user_name(membership) {
            match resolve(name) {
                Ok(profile) => members.push(profile),
                Err(reason) => unresolved.push(serde_json::json!({"member": name, "reason": reason})),
            }
        } else {
            let name = membership["member"]["name"].as_str().unwrap_or("unknown");
            unresolved.push(serde_json::json!({
                "member": name,
                "reason": "member type is not HUMAN; the People API only resolves human Google accounts",
            }));
        }
    }

    let mut result = serde_json::json!({ "members": members, "unresolved": unresolved });
    if let Some(token) = memberships_response.get("nextPageToken") {
        result["nextPageToken"] = token.clone();
    }
    result
}

fn client_error_to_cli(e: ClientError) -> CliError {
    match e {
        client::ClientError::Request(reason) => CliError::ApiRequestFailed { reason },
        client::ClientError::Status { status, body } => CliError::ApiError { status, body },
    }
}

#[cfg(test)]
#[path = "../tests/commands/spaces_tests.rs"]
mod tests;
