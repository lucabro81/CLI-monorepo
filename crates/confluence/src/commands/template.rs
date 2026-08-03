//! Handlers for the `template` command group (`create`, `list`, `update`, `delete`).
//!
//! `run_create`'s body content comes from exactly one of `--body`/`--body-file`
//! (resolved by [`resolve_body`]) — the same two non-template sources
//! `page create` offers, minus `--template-id` (a template referencing
//! another template isn't a supported concept here). The created template's
//! ID composes with the existing `page create --template-id`.
//!
//! `run_update` works around the same "no partial-patch" limitation
//! `page.rs`'s `run_update` does: `PUT /wiki/rest/api/template` requires the
//! full template (`templateId`, `name`, `templateType`, `body`) on every
//! call, so this fetches the current template first and overrides only the
//! fields `--name`/`--description`/`--body`(-file) actually supplied.
//! Unlike `page update`'s `PUT /pages/{id}`, the template ID goes in the
//! request *body* (`templateId`), not the URL path — see `client.rs`'s
//! `update_template`.

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
        TemplateCommand::Update {
            id,
            name,
            description,
            body,
            body_file,
        } => run_update(&id, name.as_deref(), description.as_deref(), body, body_file, select),
        TemplateCommand::Delete { id, confirm } => run_delete(&id, confirm, select),
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

/// Resolves `--body`/`--body-file` into an optional body override for
/// `template update` — `None` means "keep the current body", unlike
/// [`resolve_body`] (`template create`) where content is mandatory. Clap's
/// `conflicts_with` already rules out both being set.
fn resolve_optional_body(
    body: Option<String>,
    body_file: Option<String>,
) -> Result<Option<String>, CliError> {
    match (body, body_file) {
        (Some(text), None) => Ok(Some(text)),
        (None, Some(path)) => std::fs::read_to_string(&path)
            .map(Some)
            .map_err(|e| CliError::IoError {
                reason: format!("failed to read body file {path}: {e}"),
            }),
        (None, None) => Ok(None),
        (Some(_), Some(_)) => Err(CliError::Internal(
            "template update received both --body and --body-file — clap's conflicts_with \
             should have prevented this"
                .to_string(),
        )),
    }
}

fn run_update(
    id: &str,
    name: Option<&str>,
    description: Option<&str>,
    body: Option<String>,
    body_file: Option<String>,
    select: cli_fields::Select<'_>,
) -> Result<(), CliError> {
    let body_override = resolve_optional_body(body, body_file)?;

    if name.is_none() && description.is_none() && body_override.is_none() {
        return Err(CliError::TemplateUpdateMissingTarget);
    }

    let client = authenticated_client()?;
    let current = client.get_template(id).map_err(client_error_to_cli)?;

    let current_name = current["name"].as_str().unwrap_or_default();
    let current_template_type = current["templateType"].as_str().unwrap_or("page");
    let current_body = current["body"]["storage"]["value"].as_str().unwrap_or_default();

    let new_name = name.unwrap_or(current_name);
    let new_body = body_override.as_deref().unwrap_or(current_body);
    let new_description = description.or_else(|| current["description"].as_str());

    let mut request_body = json!({
        "templateId": id,
        "name": new_name,
        "templateType": current_template_type,
        "body": {"storage": {"value": new_body, "representation": "storage"}},
    });
    if let Some(description) = new_description {
        request_body["description"] = json!(description);
    }
    if let Some(space_key) = current["space"]["key"].as_str() {
        request_body["space"] = json!({"key": space_key});
    }

    let value = client
        .update_template(&request_body)
        .map_err(client_error_to_cli)?;
    print_json(&value, select)
}

/// Deletes a template. The `--confirm` check runs before
/// `authenticated_client()` — free and local, so a caller who forgot
/// `--confirm` sees the actionable error immediately rather than after a
/// network round-trip a token refresh might require.
fn run_delete(id: &str, confirm: bool, select: cli_fields::Select<'_>) -> Result<(), CliError> {
    if !confirm {
        return Err(CliError::TemplateDeleteNotConfirmed { id: id.to_string() });
    }

    authenticated_client()?
        .delete_template(id)
        .map_err(client_error_to_cli)?;

    let value = json!({"deleted": true, "id": id});
    print_json(&value, select.or_all())
}

#[cfg(test)]
#[path = "../tests/commands/template_tests.rs"]
mod tests;
