//! Handlers for the `template` command group (`create`, `list`).
//!
//! `run_create`'s body content comes from exactly one of `--body`/`--body-file`
//! (resolved by [`resolve_body`]) — the same two non-template sources
//! `page create` offers, minus `--template-id` (a template referencing
//! another template isn't a supported concept here). The created template's
//! ID composes with the existing `page create --template-id`.

use serde_json::json;

use crate::cli::TemplateCommand;
use crate::context::{authenticated_client, client_error_to_cli, print_json};
use crate::error::CliError;

pub fn run(command: TemplateCommand, select: cli_fields::Select<'_>) -> Result<(), CliError> {
    match command {
        TemplateCommand::Create {
            space_key,
            name,
            description,
            body,
            body_file,
        } => run_create(space_key, &name, description, body, body_file, select),
        TemplateCommand::List { space_key, limit, start } => {
            let value = authenticated_client()?
                .list_templates(space_key.as_deref(), limit, start)
                .map_err(client_error_to_cli)?;
            print_json(&value, select)
        }
    }
}

/// Resolves `--body`/`--body-file` into the template's body text. Clap's
/// `conflicts_with` already rules out both being set; this only needs to
/// check for neither being set.
fn resolve_body(body: Option<String>, body_file: Option<String>) -> Result<String, CliError> {
    match (body, body_file) {
        (Some(text), None) => Ok(text),
        (None, Some(path)) => std::fs::read_to_string(&path).map_err(|e| CliError::IoError {
            reason: format!("failed to read body file {path}: {e}"),
        }),
        (None, None) => Err(CliError::TemplateCreateMissingBodySource),
        (Some(_), Some(_)) => Err(CliError::Internal(
            "template create received both --body and --body-file — clap's conflicts_with \
             should have prevented this"
                .to_string(),
        )),
    }
}

fn run_create(
    space_key: Option<String>,
    name: &str,
    description: Option<String>,
    body: Option<String>,
    body_file: Option<String>,
    select: cli_fields::Select<'_>,
) -> Result<(), CliError> {
    let body_text = resolve_body(body, body_file)?;

    let mut request_body = json!({
        "name": name,
        "templateType": "page",
        "body": {"storage": {"value": body_text, "representation": "storage"}},
    });
    if let Some(description) = description {
        request_body["description"] = json!(description);
    }
    if let Some(space_key) = space_key {
        request_body["space"] = json!({"key": space_key});
    }

    let value = authenticated_client()?
        .create_template(&request_body)
        .map_err(client_error_to_cli)?;
    print_json(&value, select)
}

#[cfg(test)]
#[path = "../tests/commands/template_tests.rs"]
mod tests;
