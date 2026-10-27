//! Handler for the `space` command group (`get`, `list`).

use serde_json::Value;

use crate::cli::SpaceCommand;
use crate::auth::Identity;
use crate::client::ConfluenceClient;
use crate::context::{authenticated_client, client_error_to_cli, print_json};
use crate::error::CliError;

pub fn run(command: SpaceCommand, select: cli_fields::Select<'_>, identity: &Identity) -> Result<(), CliError> {
    match command {
        SpaceCommand::Get { key } => print_json(&get_space(&authenticated_client(identity)?, &key)?, select.or_all()),
        SpaceCommand::List { limit, cursor } => {
            let value = authenticated_client(identity)?
                .list_spaces(limit, cursor.as_deref())
                .map_err(client_error_to_cli)?;
            print_json(&value, select)
        }
    }
}

/// `space get`: the first (only) space of the key-filtered list, or
/// `SpaceNotFound` when the list is empty.
fn get_space(client: &ConfluenceClient, key: &str) -> Result<Value, CliError> {
    let mut value = client.get_space_by_key(key).map_err(client_error_to_cli)?;
    match value.get_mut("results").and_then(Value::as_array_mut) {
        Some(results) if !results.is_empty() => Ok(results.swap_remove(0)),
        _ => Err(CliError::SpaceNotFound { key: key.to_string() }),
    }
}

#[cfg(test)]
#[path = "../tests/commands/space_tests.rs"]
mod tests;
