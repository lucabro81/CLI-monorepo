//! Handler for the `user` command group.
//!
//! `search` builds a v2 `ListUsers` body from the typed flags (`build_search_body`,
//! pure and unit-tested) and prints the raw response; `--select` is mandatory
//! because the result list is unbounded.

use serde_json::{Value, json};

use crate::cli::{UserCommand, UserState};
use crate::context::{authenticated_client, client_error_to_cli, print_json};
use crate::error::CliError;

const CONTAINS_IGNORE_CASE: &str = "TEXT_QUERY_METHOD_CONTAINS_IGNORE_CASE";

pub fn run(command: UserCommand, select: cli_fields::Select<'_>) -> Result<(), CliError> {
    match command {
        UserCommand::Search {
            email,
            username,
            state,
            organization_id,
            limit,
            offset,
        } => {
            let body = build_search_body(&UserSearchFilters {
                email: email.as_deref(),
                username: username.as_deref(),
                state,
                organization_id: organization_id.as_deref(),
                limit,
                offset,
            });
            let result = authenticated_client()?
                .search_users(&body)
                .map_err(client_error_to_cli)?;
            print_json(&result, select)
        }
    }
}

pub(crate) struct UserSearchFilters<'a> {
    pub email: Option<&'a str>,
    pub username: Option<&'a str>,
    pub state: Option<UserState>,
    pub organization_id: Option<&'a str>,
    pub limit: u32,
    pub offset: u64,
}

/// Top-level `queries` entries are combined with AND by ZITADEL.
pub(crate) fn build_search_body(filters: &UserSearchFilters<'_>) -> Value {
    let mut queries = Vec::new();
    if let Some(email) = filters.email {
        queries.push(json!({"emailQuery": {"emailAddress": email, "method": CONTAINS_IGNORE_CASE}}));
    }
    if let Some(username) = filters.username {
        queries.push(json!({"userNameQuery": {"userName": username, "method": CONTAINS_IGNORE_CASE}}));
    }
    if let Some(state) = filters.state {
        queries.push(json!({"stateQuery": {"state": state_value(state)}}));
    }
    if let Some(organization_id) = filters.organization_id {
        queries.push(json!({"organizationIdQuery": {"organizationId": organization_id}}));
    }
    json!({
        "query": {"offset": filters.offset, "limit": filters.limit, "asc": true},
        "queries": queries,
    })
}

fn state_value(state: UserState) -> &'static str {
    match state {
        UserState::Active => "USER_STATE_ACTIVE",
        UserState::Inactive => "USER_STATE_INACTIVE",
        UserState::Deleted => "USER_STATE_DELETED",
        UserState::Locked => "USER_STATE_LOCKED",
        UserState::Initial => "USER_STATE_INITIAL",
    }
}

#[cfg(test)]
#[path = "../tests/commands/user_tests.rs"]
mod tests;
