//! Handler for the `user` command group.
//!
//! `search` builds a v2 `ListUsers` body from the typed flags (`build_search_body`,
//! pure and unit-tested) and prints the raw response; `--select` is mandatory
//! because the result list is unbounded. `get` prints a single user and is
//! exempt (`select.or_all()`).

use serde_json::{Value, json};

use crate::auth::Identity;
use crate::cli::{UserCommand, UserState};
use crate::context::{
    CONTAINS_IGNORE_CASE, authenticated_client, client_error_to_cli, print_json, search_query,
};
use crate::error::CliError;

pub fn run(command: UserCommand, select: cli_fields::Select<'_>, identity: &Identity) -> Result<(), CliError> {
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
            let result = authenticated_client(identity)?
                .search_users(&body)
                .map_err(client_error_to_cli)?;
            print_json(&result, select)
        }
        UserCommand::Get { user_id } => {
            let user = authenticated_client(identity)?
                .get_user(&user_id)
                .map_err(client_error_to_cli)?;
            print_json(&user, select.or_all())
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
        "query": search_query(filters.limit, filters.offset),
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
