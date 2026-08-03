//! CLI surface definition — all clap structs and enums.
//!
//! Defines the command hierarchy: `Cli` (root, holds `--select`) → `Command`
//! (top-level subcommands) → resource-specific enums (`AuthCommand`, ...).
//!
//! No logic lives here — this file is purely argument parsing and help text.
//! Every flag uses `#[arg(long)]` only; no short aliases. Complex subcommands
//! include `after_help` examples so an LLM can infer usage from a worked
//! example rather than reconstructing it from abstract parameter descriptions.

use clap::{Parser, Subcommand};

/// Confluence Cloud CLI for LLM agents — read and write Confluence pages from the command line.
#[derive(Debug, Parser)]
#[command(name = "confluence", version, about)]
pub struct Cli {
    /// Comma-separated dot-notation paths to project from the JSON output (client-side).
    /// Required on most commands: if both this and --select-all are omitted, the
    /// command fails with an error reporting the byte size of the full response and
    /// its top-level field names, so you can retry with an informed --select. A few
    /// commands whose output is always small and fixed-shape (doctor, auth whoami)
    /// are exempt and print in full regardless — see that command's own --help.
    /// This description is shared across every command and has no single
    /// correct path syntax. IMPORTANT: do NOT guess a path from this text —
    /// **scroll down to the "Examples" section of THIS command's own --help
    /// output below** for the exact paths that work with it.
    #[arg(long, global = true, value_name = "PATHS", conflicts_with = "select_all")]
    pub select: Option<String>,

    /// Explicitly print the full, unfiltered JSON response instead of specifying --select.
    /// Use when you already know the response is small; otherwise prefer --select. Still
    /// refused if the response exceeds a fixed byte cap (currently 30000 bytes) — the error
    /// reports the actual size and top-level fields so you can retry with --select instead.
    #[arg(long, global = true, conflicts_with = "select")]
    pub select_all: bool,

    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Interactive onboarding: create app.json, run auth login, verify with doctor
    ///
    /// Guides a human through setting up a 3LO OAuth 2.0 app, writes app.json,
    /// runs the interactive browser login flow, then prints a doctor JSON report
    /// as confirmation. Pass --client-id and --client-secret to skip interactive
    /// prompts (the browser login step still happens either way).
    ///
    /// If you already have a Service Account instead of a 3LO app (no human
    /// consent step, recommended for agent-driven usage), do NOT run this
    /// command: write app.json by hand
    /// (`{"client_id": "...", "client_secret": "..."}`) and run
    /// `confluence auth login` directly. See README Setup, Option A.
    #[command(after_help = "Example (interactive, 3LO app):\n  confluence init\n\nExample (non-interactive, 3LO app):\n  confluence init --client-id <ID> --client-secret <SECRET>\n\nService Account setup does not use this command — see README Setup, Option A.")]
    Init {
        /// Atlassian OAuth app client ID (skips interactive prompt if provided)
        #[arg(long)]
        client_id: Option<String>,
        /// Atlassian OAuth app client secret (skips interactive prompt if provided)
        #[arg(long)]
        client_secret: Option<String>,
    },
    /// Check that the CLI is correctly configured and can reach the Confluence API
    ///
    /// Runs four checks in order: app credentials file, stored OAuth tokens, a
    /// live API call, and the OAuth scopes granted to the token. Prints a JSON
    /// object with a status field per check. Exits non-zero if any check fails
    /// or is skipped. Always prints its full result regardless of --select —
    /// the report is generated internally and is always small and fixed-shape.
    #[command(after_help = "Examples:\n  confluence doctor\n  confluence doctor --select app_config.status,credentials.status,api.status\n\nEach check has a status field: \"ok\", \"error\", or \"skipped\".\nLater checks are skipped if an earlier one fails.")]
    Doctor,
    /// Manage authentication with Confluence
    Auth {
        #[command(subcommand)]
        command: AuthCommand,
    },
    /// Work with Confluence pages
    Page {
        #[command(subcommand)]
        command: PageCommand,
    },
    /// Work with Confluence spaces
    Space {
        #[command(subcommand)]
        command: SpaceCommand,
    },
}

#[derive(Debug, Subcommand)]
pub enum AuthCommand {
    /// Run the OAuth 2.0 login flow and store credentials locally
    ///
    /// By default runs the `client_credentials` flow for a service account: no
    /// browser, no user interaction — the access token is exchanged directly
    /// from `client_id/client_secret` in app.json. This is the expected mode for
    /// agent-driven usage.
    ///
    /// Pass --user for the interactive OAuth 2.0 (3LO) + PKCE flow for a human
    /// Atlassian account: opens the browser for consent, receives the callback
    /// on localhost:8080, exchanges the code for tokens, and stores a
    /// `refresh_token` for automatic renewal.
    ///
    /// Run this once per machine; tokens are renewed automatically after that.
    #[command(after_help = "Examples:\n  confluence auth login              # service account (client_credentials)\n  confluence auth login --user       # human account (OAuth 2.0 3LO + PKCE)\n\nRequires app.json to exist at ~/.config/confluence-cli/app.json.\nRun `confluence init` first if you have not set up the OAuth app yet.")]
    Login {
        /// Use the interactive OAuth 2.0 (3LO) + PKCE flow for a human Atlassian account
        #[arg(long)]
        user: bool,
    },
    /// Print the currently authenticated user as JSON
    ///
    /// Always prints its full result regardless of --select — an identity check,
    /// small and fixed-shape.
    #[command(after_help = "Examples:\n  confluence auth whoami\n  confluence auth whoami --select displayName,email,accountId")]
    Whoami,
}

#[derive(Debug, Subcommand)]
pub enum PageCommand {
    /// Fetch a single page by ID, including its body, and print it as JSON
    #[command(after_help = "Examples:\n  confluence page get 123456\n  confluence page get 123456 --select title,body.storage.value,version.number")]
    Get {
        /// Page ID
        id: String,
    },
    /// Create a new page in a space
    ///
    /// Exactly one of --body, --template-file, or --template-id supplies the
    /// page content. --body is raw Confluence storage-format XHTML (the same
    /// format `page get`'s body.storage.value returns) — plain text with no
    /// markup is also valid storage format. --template-file reads that
    /// content from a local file instead of a command-line argument.
    /// --template-id copies the body of an existing Confluence content
    /// template (find one's ID via the Confluence UI: Space settings ->
    /// Content Types -> Templates) — Confluence has no API to create a page
    /// "from" a template directly, so this fetches the template's body and
    /// submits it as this page's initial content, same as duplicating it by
    /// hand.
    #[command(after_help = "Examples:\n  confluence page create --space-id 98765 --title \"Sprint Notes\" --body \"<p>Agenda</p>\"\n  confluence page create --space-id 98765 --title \"Runbook\" --template-file ./runbook-template.html\n  confluence page create --space-id 98765 --title \"Retro\" --template-id 4321 --parent-id 111222")]
    Create {
        /// Numeric ID of the space to create the page in — find one with `space list`
        #[arg(long)]
        space_id: String,
        /// Page title
        #[arg(long)]
        title: String,
        /// Optional parent page ID, to create this page as a child of another
        #[arg(long)]
        parent_id: Option<String>,
        /// Page body as raw Confluence storage-format XHTML
        #[arg(long, conflicts_with_all = ["template_file", "template_id"])]
        body: Option<String>,
        /// Path to a local file whose content becomes the page body
        #[arg(long, conflicts_with_all = ["body", "template_id"])]
        template_file: Option<String>,
        /// ID of an existing Confluence content template to copy as the page body
        #[arg(long, conflicts_with_all = ["body", "template_file"])]
        template_id: Option<String>,
    },
    /// Update an existing page's title and/or body
    ///
    /// Confluence's v2 API replaces the whole page on every update — there is
    /// no partial-patch endpoint. This command fetches the page's current
    /// title, body, and version number first, then submits a full update with
    /// your --title and/or --body overriding just those fields (the other
    /// keeps its current value) and the version number incremented by one.
    #[command(after_help = "Examples:\n  confluence page update 123456 --title \"Sprint Notes (updated)\"\n  confluence page update 123456 --body \"<p>New agenda</p>\"\n\nAt least one of --title or --body is required.")]
    Update {
        /// Page ID
        id: String,
        /// New page title
        #[arg(long)]
        title: Option<String>,
        /// New page body as raw Confluence storage-format XHTML
        #[arg(long)]
        body: Option<String>,
    },
    /// Search Confluence content using CQL (Confluence Query Language) and print matches as JSON
    #[command(after_help = "Examples:\n  confluence page search --cql \"type=page AND space=ENG AND title~\\\"Runbook\\\"\"\n  confluence page search --cql \"type=page AND space=ENG\" --limit 10\n  confluence page search --cql \"type=page\" --start 25\n\nPagination: the response's size field tells you how many results this page\nreturned; pass --start <previous start + limit> to fetch the next page.")]
    Search {
        /// CQL query string, e.g. "type=page AND space=ENG"
        #[arg(long)]
        cql: String,
        /// Maximum number of results to return (default: 25)
        #[arg(long, default_value = "25")]
        limit: u32,
        /// Offset into the result set, for pagination (default: 0)
        #[arg(long, default_value = "0")]
        start: u32,
    },
}

#[derive(Debug, Subcommand)]
pub enum SpaceCommand {
    /// List Confluence spaces and print them as JSON
    #[command(after_help = "Examples:\n  confluence space list\n  confluence space list --limit 10\n  confluence space list --cursor <cursor-from-previous-response>\n\nPagination: the response's _links.next field (if present) contains a cursor\nquery parameter — pass its value to --cursor to fetch the next page.")]
    List {
        /// Maximum number of spaces to return (default: 25)
        #[arg(long, default_value = "25")]
        limit: u32,
        /// Cursor token for the next page, from the _links.next field of a previous response
        #[arg(long)]
        cursor: Option<String>,
    },
}

#[cfg(test)]
#[path = "tests/cli_tests.rs"]
mod tests;
