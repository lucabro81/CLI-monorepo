//! Handler for the `project` command group.
//!
//! `list` builds a v2 `ListProjects` body (`build_list_body`, pure and
//! unit-tested) and prints the raw response under mandatory `--select`. Unlike
//! the user/organization services, `ProjectService` uses the newer request shape
//! (`pagination` + `filters`, `TEXT_FILTER_METHOD_*`) and response shape
//! (`pagination.totalResult` + `projects`).

use serde_json::{Value, json};

use crate::cli::ProjectCommand;
use crate::context::{authenticated_client, client_error_to_cli, print_json, search_query};
use crate::error::CliError;

const FILTER_CONTAINS_IGNORE_CASE: &str = "TEXT_FILTER_METHOD_CONTAINS_IGNORE_CASE";

pub fn run(command: ProjectCommand, select: cli_fields::Select<'_>) -> Result<(), CliError> {
    match command {
        ProjectCommand::List {
            name,
            organization_id,
            limit,
            offset,
        } => {
            let body = build_list_body(name.as_deref(), organization_id.as_deref(), limit, offset);
            let result = authenticated_client()?
                .list_projects(&body)
                .map_err(client_error_to_cli)?;
            print_json(&result, select)
        }
    }
}

pub(crate) fn build_list_body(
    name: Option<&str>,
    organization_id: Option<&str>,
    limit: u32,
    offset: u64,
) -> Value {
    let mut filters = Vec::new();
    if let Some(name) = name {
        filters.push(json!({"projectNameFilter": {"projectName": name, "method": FILTER_CONTAINS_IGNORE_CASE}}));
    }
    if let Some(organization_id) = organization_id {
        filters.push(json!({"organizationIdFilter": {"organizationId": organization_id}}));
    }
    json!({"pagination": search_query(limit, offset), "filters": filters})
}

#[cfg(test)]
#[path = "../tests/commands/project_tests.rs"]
mod tests;
