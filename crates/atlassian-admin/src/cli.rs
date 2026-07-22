//! CLI surface definition — all clap structs and enums.
//!
//! No logic lives here — this file is purely argument parsing and help text.
//! Every flag uses `#[arg(long)]` only; no short aliases.

use clap::{Parser, Subcommand};

/// Atlassian Organization Admin API CLI for LLM agents — resolve an Atlassian
/// `account_id` to a managed-account profile/email.
#[derive(Debug, Parser)]
#[command(name = "atlassian-admin", version, about)]
pub struct Cli {
    /// Comma-separated dot-notation paths to project from the JSON output (client-side).
    /// Required on most commands: if both this and --select-all are omitted, the
    /// command fails with an error reporting the byte size of the full response and
    /// its top-level field names, so you can retry with an informed --select. Commands
    /// whose output is always small and fixed-shape (doctor, user get) are exempt and
    /// print in full regardless — see that command's own --help.
    /// Example: --select `email,name`
    #[arg(long, global = true, value_name = "PATHS", conflicts_with = "select_all")]
    pub select: Option<String>,

    /// Explicitly print the full, unfiltered JSON response instead of specifying --select.
    /// Use when you already know the response is small; otherwise prefer --select.
    #[arg(long, global = true, conflicts_with = "select")]
    pub select_all: bool,

    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Write app.json with your Organization API key and org id
    ///
    /// Unlike other crates' init, this does not fall back to an interactive
    /// stdin prompt if you omit the flags — an Organization API key is too
    /// sensitive to risk landing in terminal scrollback or session logs.
    /// Passing both flags writes app.json directly and runs doctor as
    /// verification; passing neither (or only one) creates an empty skeleton
    /// file and prints its path for you to fill in by hand.
    #[command(after_help = "Examples:\n  atlassian-admin init\n  atlassian-admin init --api-key <KEY> --org-id <ORG_ID>")]
    Init {
        /// Organization API key from admin.atlassian.com (skips the skeleton-file
        /// path and writes app.json directly if provided together with --org-id)
        #[arg(long)]
        api_key: Option<String>,
        /// Organization ID shown alongside the API key when it was created
        #[arg(long)]
        org_id: Option<String>,
    },
    /// Check that the CLI is correctly configured and can reach the Atlassian Admin API
    ///
    /// Always prints its full result regardless of --select — the report is
    /// generated internally and is always small and fixed-shape.
    #[command(after_help = "Example:\n  atlassian-admin doctor")]
    Doctor,
    /// Resolve an Atlassian `account_id` to a profile
    User {
        #[command(subcommand)]
        command: UserCommand,
    },
}

#[derive(Debug, Subcommand)]
pub enum UserCommand {
    /// Resolve an Atlassian `account_id` to a managed-account profile (including email), as JSON
    ///
    /// Always prints its full result regardless of --select — a single profile
    /// object, fixed-shape, wrapped under an "account" key. Only resolves
    /// accounts managed under your organization (email domain verified via
    /// Atlassian Access/Guard). Requires an unscoped ("without scopes") API key.
    #[command(after_help = "Examples:\n  atlassian-admin user get --account-id 5b10a2844c20165700ede21g\n  atlassian-admin user get --account-id 5b10a2844c20165700ede21g --select account.email,account.name")]
    Get {
        /// Atlassian `account_id` (the identity shared across Jira, Confluence, and Bitbucket)
        #[arg(long)]
        account_id: String,
    },
    /// List every managed user in the organization, as JSON
    ///
    /// Each entry already includes `account_id`/`name`/`email` directly — no
    /// need to call `user get` per person. Paginated; if the response's
    /// `links` includes a `next` URL, pass its `cursor` query value to
    /// --cursor to fetch the next page.
    #[command(after_help = "Examples:\n  atlassian-admin user list\n  atlassian-admin user list --select data\n  atlassian-admin user list --cursor eyJvZmZzZXQiOjUwfQ")]
    List {
        /// Opaque pagination cursor from a previous response's links.next URL
        #[arg(long)]
        cursor: Option<String>,
    },
}

#[cfg(test)]
#[path = "tests/cli_tests.rs"]
mod tests;
