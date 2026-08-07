//! Handler for the `branch` command group.

use crate::cli::BranchCommand;
use crate::context::{authenticated_client, print_json, split_repository};
use crate::error::CliError;

/// Dispatches a `BranchCommand` variant to the appropriate Bitbucket API call.
pub fn run(command: BranchCommand, select: cli_fields::Select<'_>) -> Result<(), CliError> {
    match command {
        BranchCommand::List { repository, page } => {
            let (workspace, repo_slug) = split_repository(&repository)?;
            let value = authenticated_client()?
                .list_branches(workspace, repo_slug, page)
                .map_err(|e| CliError::ApiRequestFailed {
                    reason: e.to_string(),
                })?;
            print_json(&value, select)
        }
        BranchCommand::Create { repository, name, target } => {
            let (workspace, repo_slug) = split_repository(&repository)?;
            let body = serde_json::json!({
                "name": name,
                "target": { "hash": target }
            });
            let value = authenticated_client()?
                .create_branch(workspace, repo_slug, &body)
                .map_err(|e| CliError::ApiRequestFailed {
                    reason: e.to_string(),
                })?;
            // Exempt: a single branch object, fixed shape.
            print_json(&value, select.or_all())
        }
        BranchCommand::SuggestName { issue_key, issue_type, issue_summary, repository, prefix } => {
            let desired_kind = infer_kind(&issue_type);
            let (prefix, prefix_source) = if let Some(prefix) = prefix {
                (prefix, "override")
            } else if let Some(repository) = repository {
                let (workspace, repo_slug) = split_repository(&repository)?;
                let model = authenticated_client()?
                    .get_branching_model(workspace, repo_slug)
                    .map_err(|e| CliError::ApiRequestFailed {
                        reason: e.to_string(),
                    })?;
                match resolve_prefix_from_branching_model(&model, &desired_kind) {
                    Some(prefix) => (prefix, "branching_model"),
                    None => (desired_kind.clone(), "heuristic"),
                }
            } else {
                (desired_kind.clone(), "heuristic")
            };
            let slug = slugify(&issue_summary);
            let name = build_branch_name(&prefix, &issue_key, &slug);
            let value = serde_json::json!({
                "name": name,
                "prefix": prefix,
                "prefix_source": prefix_source,
                "slug": slug,
                "issue_key": issue_key,
            });
            // Exempt: a small fixed-shape object.
            print_json(&value, select.or_all())
        }
    }
}

/// Maps a Jira issue type to a Bitbucket branch kind. `Bug` (case-insensitive)
/// maps to `bugfix`; every other issue type falls back to `feature`, since
/// Bitbucket has no default kind corresponding to Jira's other built-in types
/// (Task, Story, Epic, ...).
fn infer_kind(issue_type: &str) -> String {
    if issue_type.eq_ignore_ascii_case("bug") {
        "bugfix".to_string()
    } else {
        "feature".to_string()
    }
}

/// Looks up `desired_kind` in a repository's branching model response
/// (`GET .../branching-model`), returning its configured prefix (trailing
/// `/` stripped) if that kind is present and enabled. Falls back once to
/// `feature` if `desired_kind` isn't usable; returns `None` if neither is
/// available, leaving the caller to fall back to the offline heuristic.
fn resolve_prefix_from_branching_model(model: &serde_json::Value, desired_kind: &str) -> Option<String> {
    let branch_types = model.get("branch_types")?.as_array()?;
    let find = |kind: &str| -> Option<String> {
        branch_types.iter().find_map(|branch_type| {
            let matches_kind = branch_type.get("kind")?.as_str()? == kind;
            let enabled = branch_type.get("enabled")?.as_bool()?;
            if matches_kind && enabled {
                let prefix = branch_type.get("prefix")?.as_str()?;
                Some(prefix.trim_end_matches('/').to_string())
            } else {
                None
            }
        })
    };

    find(desired_kind).or_else(|| {
        if desired_kind == "feature" {
            None
        } else {
            find("feature")
        }
    })
}

/// Maximum length, in characters, of the slug portion of a suggested branch name.
const MAX_SLUG_LEN: usize = 60;

/// Normalizes a Jira issue summary into a branch-name-safe slug: lowercased,
/// runs of non-ASCII-alphanumeric characters collapsed into single hyphens,
/// leading/trailing hyphens trimmed, truncated to `MAX_SLUG_LEN` characters.
/// Non-ASCII letters (accents, etc.) are dropped, not transliterated.
fn slugify(summary: &str) -> String {
    let mut slug = String::new();
    let mut last_was_separator = true;
    for ch in summary.to_lowercase().chars() {
        if ch.is_ascii_alphanumeric() {
            slug.push(ch);
            last_was_separator = false;
        } else if !last_was_separator {
            slug.push('-');
            last_was_separator = true;
        }
    }
    if slug.ends_with('-') {
        slug.pop();
    }

    if slug.chars().count() > MAX_SLUG_LEN {
        slug = slug.chars().take(MAX_SLUG_LEN).collect();
        while slug.ends_with('-') {
            slug.pop();
        }
    }

    slug
}

/// Assembles the final suggested branch name from its parts, omitting the
/// `prefix/` segment when `prefix` is empty and the trailing `-slug` when
/// `slug` is empty.
fn build_branch_name(prefix: &str, issue_key: &str, slug: &str) -> String {
    let key_part = if slug.is_empty() {
        issue_key.to_string()
    } else {
        format!("{issue_key}-{slug}")
    };

    if prefix.is_empty() {
        key_part
    } else {
        format!("{prefix}/{key_part}")
    }
}

#[cfg(test)]
#[path = "../tests/commands/branch_tests.rs"]
mod tests;
