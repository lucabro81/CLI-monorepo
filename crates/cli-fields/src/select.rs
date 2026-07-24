//! Resolves the `--select` / `--select-all` global flags into a single
//! `Select` value once at startup, and renders a JSON `Value` to a
//! pretty-printed string according to it.
//!
//! `--select` is effectively mandatory: omitting both flags causes
//! `render_json` to refuse to print the (potentially huge) response and
//! instead return an actionable error reporting its byte size and top-level
//! field names, so the caller can retry with an informed `--select`.
//! `--select-all` is the explicit, stateless opt-out — passing it is itself
//! the caller's confirmation that printing the full response is fine,
//! mirroring the `--confirm` pattern already used for destructive commands.
//! That confirmation isn't unconditional, though: even `--select-all` refuses
//! to print a response over `MAX_ALL_BYTES`, since a caller can't have
//! meaningfully confirmed "fine to flood my own context window" — the error
//! pushes them back to `--select` once the response is clearly too large.

use crate::fields::{describe_top_level_shape, filter_fields};
use serde_json::Value;

/// Byte cap (pretty-printed) enforced on `Select::All`, even though it's an
/// explicit opt-out. A caller who reaches for --select-all without knowing
/// the response size can otherwise flood their own context window; this
/// forces a fallback to --select once the response is clearly too large for
/// that to have been a reasonable choice.
pub const MAX_ALL_BYTES: usize = 30_000;

/// What the caller decided about field selection, resolved once from CLI flags.
#[derive(Debug, Clone, Copy)]
pub enum Select<'a> {
    /// Neither --select nor --select-all was passed: `render_json` will refuse to print.
    Required,
    /// --select-all was passed: print the full response, no filtering, deliberately.
    All,
    /// --select <paths> was passed: filter to these dot-notation paths.
    Fields(&'a [&'a str]),
}

impl<'a> Select<'a> {
    /// For commands exempt from the mandatory-select requirement (output that is
    /// either synthesized by the CLI itself or a small, fixed-shape API response):
    /// turns `Required` into `All` so the command always prints in full, while an
    /// explicit `--select`/`--select-all` from the caller is still honored as-is.
    #[must_use]
    pub fn or_all(self) -> Select<'a> {
        match self {
            Select::Required => Select::All,
            other => other,
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum RenderError {
    #[error(
        "refusing to print the full {size}-byte JSON response without --select — an omitted \
        --select would likely flood the caller's context window. {available_fields}. \
        Retry with --select and one or more dot-notation paths built from the fields above \
        (e.g. --select fieldName or --select fieldName.nestedField), or pass --select-all to \
        explicitly confirm printing the unfiltered response."
    )]
    SelectRequired {
        size: usize,
        available_fields: String,
    },

    #[error(
        "refusing to print the full {size}-byte JSON response even with --select-all — it \
        exceeds the {max}-byte cap and would likely flood the caller's context window. \
        {available_fields}. Retry with --select and one or more dot-notation paths built from \
        the fields above (e.g. --select fieldName or --select fieldName.nestedField) to narrow \
        the response instead."
    )]
    AllTooLarge {
        size: usize,
        max: usize,
        available_fields: String,
    },

    #[error("failed to serialize response to JSON: {0}")]
    Serialize(String),
}

/// Renders `value` to a pretty-printed JSON string according to `select`.
pub fn render_json(value: &Value, select: Select<'_>) -> Result<String, RenderError> {
    match select {
        Select::Required => {
            let pretty = serde_json::to_string_pretty(value)
                .map_err(|e| RenderError::Serialize(e.to_string()))?;
            Err(RenderError::SelectRequired {
                size: pretty.len(),
                available_fields: describe_top_level_shape(value),
            })
        }
        Select::All => {
            let pretty = serde_json::to_string_pretty(value)
                .map_err(|e| RenderError::Serialize(e.to_string()))?;
            if pretty.len() > MAX_ALL_BYTES {
                return Err(RenderError::AllTooLarge {
                    size: pretty.len(),
                    max: MAX_ALL_BYTES,
                    available_fields: describe_top_level_shape(value),
                });
            }
            Ok(pretty)
        }
        Select::Fields(fields) => {
            let filtered = filter_fields(value.clone(), fields);
            serde_json::to_string_pretty(&filtered)
                .map_err(|e| RenderError::Serialize(e.to_string()))
        }
    }
}

#[cfg(test)]
#[path = "tests/select_tests.rs"]
mod tests;
