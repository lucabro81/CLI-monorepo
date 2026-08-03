//! Handlers for the `page` command group (`get`, `create`, `update`, `search`, `delete`).
//!
//! `run_create`'s body content comes from exactly one of three sources —
//! `--body` (raw text), `--body-file` (the same kind of content, read from a
//! local file — unrelated to Confluence's own Template feature), or
//! `--template-id` (an existing Confluence template's body, fetched via the
//! v1 template API) — resolved by [`parse_body_source`]. Confluence has no
//! API to create a page "from" a template directly (there IS a documented
//! `POST /wiki/rest/api/template` to create the template *object* itself,
//! confirmed via developer.atlassian.com — the gap is specifically "create a
//! page pre-filled from template X in one call"); the only way is to fetch
//! the template's stored body and submit it as a normal page body, same as
//! what this command does.
//!
//! `run_update` works around Confluence v2's lack of a partial-patch
//! endpoint: it fetches the page's current title/body/version first, so a
//! caller can override just `--title` or just `--body` without needing to
//! resupply the other.
//!
//! `run_delete` moves a page to the trash by default; `--purge` permanently
//! removes it, but only works on a page that's already trashed.

use serde_json::json;

use crate::cli::PageCommand;
use crate::context::{authenticated_client, client_error_to_cli, print_json};
use crate::error::CliError;

pub fn run(command: PageCommand, select: cli_fields::Select<'_>) -> Result<(), CliError> {
    match command {
        PageCommand::Get { id } => {
            let value = authenticated_client()?
                .get_page(&id)
                .map_err(client_error_to_cli)?;
            print_json(&value, select)
        }
        PageCommand::Create {
            space_id,
            title,
            parent_id,
            body,
            body_file,
            template_id,
        } => run_create(&space_id, &title, parent_id, body, body_file, template_id, select),
        PageCommand::Update { id, title, body } => {
            run_update(&id, title.as_deref(), body.as_deref(), select)
        }
        PageCommand::Search { cql, limit, start } => {
            let value = authenticated_client()?
                .search_content(&cql, limit, start)
                .map_err(client_error_to_cli)?;
            print_json(&value, select)
        }
        PageCommand::Delete { id, confirm, purge } => run_delete(&id, confirm, purge, select),
    }
}

/// Exactly one content source for a new page's body.
#[derive(Debug, PartialEq, Eq)]
enum BodySource {
    Body(String),
    BodyFile(String),
    TemplateId(String),
}

/// Resolves `--body`/`--body-file`/`--template-id` into exactly one
/// [`BodySource`]. Clap's `conflicts_with_all` already rules out more than
/// one being set; this only needs to check for none being set.
fn parse_body_source(
    body: Option<String>,
    body_file: Option<String>,
    template_id: Option<String>,
) -> Result<BodySource, CliError> {
    match (body, body_file, template_id) {
        (Some(b), None, None) => Ok(BodySource::Body(b)),
        (None, Some(f), None) => Ok(BodySource::BodyFile(f)),
        (None, None, Some(t)) => Ok(BodySource::TemplateId(t)),
        (None, None, None) => Err(CliError::PageCreateMissingBodySource),
        _ => Err(CliError::Internal(
            "page create received more than one body source — clap's conflicts_with_all \
             should have prevented this"
                .to_string(),
        )),
    }
}

fn run_create(
    space_id: &str,
    title: &str,
    parent_id: Option<String>,
    body: Option<String>,
    body_file: Option<String>,
    template_id: Option<String>,
    select: cli_fields::Select<'_>,
) -> Result<(), CliError> {
    let source = parse_body_source(body, body_file, template_id)?;
    let client = authenticated_client()?;

    let resolved_body = match source {
        BodySource::Body(text) => text,
        BodySource::BodyFile(path) => {
            std::fs::read_to_string(&path).map_err(|e| CliError::IoError {
                reason: format!("failed to read body file {path}: {e}"),
            })?
        }
        BodySource::TemplateId(template_id) => {
            let template = client
                .get_template(&template_id)
                .map_err(client_error_to_cli)?;
            template["body"]["storage"]["value"]
                .as_str()
                .map(str::to_string)
                .ok_or_else(|| CliError::TemplateBodyMissing {
                    reason: format!("template {template_id}"),
                })?
        }
    };

    let mut request_body = json!({
        "spaceId": space_id,
        "status": "current",
        "title": title,
        "body": {"representation": "storage", "value": resolved_body},
    });
    if let Some(parent_id) = parent_id {
        request_body["parentId"] = json!(parent_id);
    }

    let value = client.create_page(&request_body).map_err(client_error_to_cli)?;
    print_json(&value, select)
}

/// At least one of `--title`/`--body` is required for `page update` —
/// otherwise the update has nothing to change. Clap cannot express "at least
/// one of these two optional flags" declaratively, so this runtime check
/// mirrors jira's `validate_assign_target` pattern.
fn validate_update_target(title: Option<&str>, body: Option<&str>) -> Result<(), CliError> {
    if title.is_none() && body.is_none() {
        return Err(CliError::PageUpdateMissingTarget);
    }
    Ok(())
}

fn run_update(
    id: &str,
    title: Option<&str>,
    body: Option<&str>,
    select: cli_fields::Select<'_>,
) -> Result<(), CliError> {
    validate_update_target(title, body)?;

    let client = authenticated_client()?;
    let current = client.get_page(id).map_err(client_error_to_cli)?;

    let current_title = current["title"].as_str().unwrap_or_default();
    let current_body = current["body"]["storage"]["value"].as_str().unwrap_or_default();
    let current_version = current["version"]["number"].as_u64().unwrap_or_default();

    let new_title = title.unwrap_or(current_title);
    let new_body = body.unwrap_or(current_body);

    let request_body = json!({
        "id": id,
        "status": "current",
        "title": new_title,
        "body": {"representation": "storage", "value": new_body},
        "version": {"number": current_version + 1},
    });

    let value = client
        .update_page(id, &request_body)
        .map_err(client_error_to_cli)?;
    print_json(&value, select)
}

/// Deletes (or, with `purge`, permanently removes) a page. The `--confirm`
/// check runs before `authenticated_client()` — free and local, so a caller
/// who forgot `--confirm` sees the actionable error immediately rather than
/// after a network round-trip a token refresh might require (same reasoning
/// as jira's `issue delete`).
fn run_delete(
    id: &str,
    confirm: bool,
    purge: bool,
    select: cli_fields::Select<'_>,
) -> Result<(), CliError> {
    if !confirm {
        return Err(CliError::PageDeleteNotConfirmed { id: id.to_string() });
    }

    authenticated_client()?
        .delete_page(id, purge)
        .map_err(client_error_to_cli)?;

    let value = json!({"deleted": true, "id": id, "purged": purge});
    print_json(&value, select.or_all())
}

#[cfg(test)]
#[path = "../tests/commands/page_tests.rs"]
mod tests;
