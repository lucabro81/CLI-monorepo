//! Shared `--select` field-projection support for this workspace's LLM-facing
//! CLIs (jira, bitbucket, google-chat). Extracted because all three crates
//! implemented the exact same dot-notation JSON projection independently —
//! see this workspace's root CLAUDE.md "Shared library: crates/cli-fields"
//! section for the extraction rationale.

mod fields;
mod select;

pub use fields::{describe_top_level_shape, filter_fields};
pub use select::{render_json, RenderError, Select, MAX_ALL_BYTES};
