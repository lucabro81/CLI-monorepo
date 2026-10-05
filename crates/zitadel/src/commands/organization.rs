//! Handler for the `organization` command group.
//!
//! `list` builds a v2 `ListOrganizations` body (`build_list_body`, pure and
//! unit-tested) and prints the raw response under mandatory `--select`.

use serde_json::{Value, json};

use crate::cli::OrganizationCommand;
use crate::context::{
    CONTAINS_IGNORE_CASE, authenticated_client, client_error_to_cli, print_json, search_query,
};
use crate::error::CliError;

pub fn run(command: OrganizationCommand, select: cli_fields::Select<'_>) -> Result<(), CliError> {
    match command {
        OrganizationCommand::List { name, limit, offset } => {
            let body = build_list_body(name.as_deref(), limit, offset);
            let result = authenticated_client()?
                .list_organizations(&body)
                .map_err(client_error_to_cli)?;
            print_json(&result, select)
        }
    }
}

pub(crate) fn build_list_body(name: Option<&str>, limit: u32, offset: u64) -> Value {
    let queries: Vec<Value> = name
        .map(|name| json!({"nameQuery": {"name": name, "method": CONTAINS_IGNORE_CASE}}))
        .into_iter()
        .collect();
    json!({"query": search_query(limit, offset), "queries": queries})
}

#[cfg(test)]
#[path = "../tests/commands/organization_tests.rs"]
mod tests;
