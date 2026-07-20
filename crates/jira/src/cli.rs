//! CLI surface definition — all clap structs and enums.
//!
//! Defines the full command hierarchy: `Cli` (root, holds `--select`) →
//! `Command` (top-level subcommands) → resource-specific enums
//! (`AuthCommand`, `IssueCommand`, `CommentCommand`).
//!
//! No logic lives here — this file is purely argument parsing and help text.
//! Every flag uses `#[arg(long)]` only; no short aliases. Complex subcommands
//! include `after_help` examples so an LLM can infer usage from a worked
//! example rather than reconstructing it from abstract parameter descriptions.

use clap::{Parser, Subcommand};

/// Jira CLI for LLM agents — query Jira issues from the command line.
#[derive(Debug, Parser)]
#[command(name = "jira", version, about)]
pub struct Cli {
    /// Comma-separated dot-notation paths to project from the JSON output (client-side).
    /// Required on most commands: if both this and --select-all are omitted, the
    /// command fails with an error reporting the byte size of the full response and
    /// its top-level field names, so you can retry with an informed --select. A few
    /// commands whose output is always small and fixed-shape (doctor, auth whoami,
    /// issue create/delete/transitions/transition/comment add/comment remove) are
    /// exempt and print in full regardless — see that command's own --help.
    /// Example: --select summary,status.name,assignee.displayName
    /// Example: --select transitions.id,transitions.name
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
    /// Interactive onboarding: create app.json, run auth login, verify with doctor
    ///
    /// Guides a human through setting up the Atlassian OAuth 2.0 app, writes app.json,
    /// runs the login flow, then prints a doctor JSON report as confirmation.
    /// Pass --client-id and --client-secret to skip interactive prompts.
    #[command(after_help = "Example (interactive):\n  jira init\n\nExample (non-interactive):\n  jira init --client-id <ID> --client-secret <SECRET>")]
    Init {
        /// Atlassian OAuth app client ID (skips interactive prompt if provided)
        #[arg(long)]
        client_id: Option<String>,
        /// Atlassian OAuth app client secret (skips interactive prompt if provided)
        #[arg(long)]
        client_secret: Option<String>,
    },
    /// Check that the CLI is correctly configured and can reach the Jira API
    ///
    /// Runs three checks in order: app credentials file, stored OAuth tokens,
    /// and a live API call. Prints a JSON object with a status field per check.
    /// Exits non-zero if any check fails or is skipped. Always prints its full
    /// result regardless of --select — the report is generated internally and is
    /// always small and fixed-shape.
    #[command(after_help = "Examples:\n  jira doctor\n  jira doctor --select app_config.status,credentials.status,api.status\n\nEach check has a status field: \"ok\", \"error\", or \"skipped\".\nLater checks are skipped if an earlier one fails.")]
    Doctor,
    /// Manage authentication with Jira
    Auth {
        #[command(subcommand)]
        command: AuthCommand,
    },
    /// Work with Jira issues
    Issue {
        #[command(subcommand)]
        command: IssueCommand,
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
    #[command(after_help = "Examples:\n  jira auth login              # service account (client_credentials)\n  jira auth login --user       # human account (OAuth 2.0 3LO + PKCE)\n\nRequires app.json to exist at ~/.config/jira-cli/app.json.\nRun `jira init` first if you have not set up the OAuth app yet.")]
    Login {
        /// Use the interactive OAuth 2.0 (3LO) + PKCE flow for a human Atlassian account
        #[arg(long)]
        user: bool,
    },
    /// Print the currently authenticated user as JSON
    ///
    /// Always prints its full result regardless of --select — an identity check,
    /// small and fixed-shape.
    #[command(after_help = "Examples:\n  jira auth whoami\n  jira auth whoami --select displayName,emailAddress,accountId")]
    Whoami,
}

#[derive(Debug, Subcommand)]
pub enum IssueCommand {
    /// Fetch a single issue by key (e.g. PROJ-123) and print it as JSON
    #[command(after_help = "Examples:\n  jira issue get PROJ-123\n  jira issue get PROJ-123 --select summary,status.name,assignee.displayName,priority.name")]
    Get {
        /// Issue key, e.g. PROJ-123
        key: String,
    },
    /// Manage comments on a Jira issue
    Comment {
        #[command(subcommand)]
        command: CommentCommand,
    },
    /// List the workflow transitions available for an issue in its current state, as JSON
    ///
    /// Always prints its full result regardless of --select — a bounded list of
    /// workflow states, small and fixed-shape.
    #[command(after_help = "Examples:\n  jira issue transitions PROJ-123\n  jira issue transitions PROJ-123 --select transitions.id,transitions.name\n\nUse the transition names returned here as the --to argument for `issue transition`.")]
    Transitions {
        /// Issue key, e.g. PROJ-123
        key: String,
    },
    /// Search issues using JQL (Jira Query Language) and return matching issues as JSON
    #[command(after_help = "Examples:\n  jira issue search --jql \"project=KAN AND status=\\\"In Progress\\\"\"\n  jira issue search --jql \"assignee=currentUser() ORDER BY created DESC\" --max-results 10\n  jira issue search --jql \"project=KAN\" --fields summary,status,priority\n  jira issue search --jql \"project=KAN AND status!=Done\" --stale-days 14\n\nPagination: the response includes a nextPageToken field when more results exist.\nPass its value to --page-token on the next call to fetch the following page.\n\n--stale-days N adds \"AND updated <= -Nd\" to --jql (inserted before ORDER BY, if present) to\nfind issues that have not been updated in at least N days.")]
    Search {
        /// JQL query string, e.g. "project=KAN AND status=\"Done\""
        #[arg(long)]
        jql: String,
        /// Maximum number of issues to return (default: 50, max: 100)
        #[arg(long, default_value = "50")]
        max_results: u32,
        /// Cursor token for the next page, from the nextPageToken field of a previous response
        #[arg(long)]
        page_token: Option<String>,
        /// Comma-separated Jira field names to include in each issue (server-side).
        /// Reduces response size. Use *all for every field, *navigable for navigable fields.
        /// Example: --fields summary,status,assignee,priority
        #[arg(long)]
        fields: Option<String>,
        /// Only include issues not updated in at least N days. Adds "AND updated <= -Nd"
        /// to --jql server-side (JQL's own relative-date syntax — no separate API needed).
        #[arg(long)]
        stale_days: Option<u32>,
    },
    /// Create a new issue in a Jira project
    ///
    /// Always prints its full result regardless of --select — Jira's create
    /// response is only {id, key, self}, small and fixed-shape.
    #[command(after_help = "Examples:\n  jira issue create --project KAN --type Task --summary \"Fix login bug\"\n  jira issue create --project KAN --type Bug --summary \"Crash on startup\" --description \"Happens on macOS 14\" --priority High")]
    Create {
        /// Project key, e.g. KAN
        #[arg(long)]
        project: String,
        /// Issue type name, e.g. Task, Bug, Story
        #[arg(long = "type")]
        issue_type: String,
        /// One-line summary of the issue
        #[arg(long)]
        summary: String,
        /// Optional description (plain text; converted to Jira document format)
        #[arg(long)]
        description: Option<String>,
        /// Optional assignee account ID (use `auth whoami` to get your own)
        #[arg(long)]
        assignee: Option<String>,
        /// Optional priority name, e.g. High, Medium, Low
        #[arg(long)]
        priority: Option<String>,
    },
    /// Permanently delete an issue — requires --confirm
    ///
    /// Always prints its full result regardless of --select — a small, synthesized
    /// confirmation object.
    #[command(after_help = "Example: jira issue delete KAN-5 --confirm\n\nThis action is irreversible. --confirm must be passed explicitly so the caller acknowledges the deletion. Pass --delete-subtasks if the issue has subtasks, otherwise Jira will refuse the request.")]
    Delete {
        /// Issue key to delete, e.g. PROJ-123
        key: String,
        /// Acknowledge that this action is permanent and irreversible
        #[arg(long)]
        confirm: bool,
        /// Also delete subtasks; required if the issue has any
        #[arg(long)]
        delete_subtasks: bool,
    },
    /// Move an issue to a different status via a workflow transition
    ///
    /// Always prints its full result regardless of --select — a small, synthesized
    /// confirmation object.
    #[command(after_help = "Example: jira issue transition KAN-4 --to \"In Progress\"\n\nUse the exact status name as it appears in Jira. If the name does not match any available transition, the command fails and lists the valid options.")]
    Transition {
        /// Issue key, e.g. PROJ-123
        key: String,
        /// Target status name, e.g. \"In Progress\" or \"Done\"
        #[arg(long)]
        to: String,
    },
}

#[derive(Debug, Subcommand)]
pub enum CommentCommand {
    /// Add a comment to an issue and print the created comment as JSON
    ///
    /// Always prints its full result regardless of --select — a single comment
    /// object, small and fixed-shape.
    #[command(after_help = "Example: jira issue comment add KAN-4 --body \"Blocked by network issue, retrying tomorrow\"")]
    Add {
        /// Issue key, e.g. PROJ-123
        key: String,
        /// Comment text (plain text; the CLI converts it to Jira's document format)
        #[arg(long)]
        body: String,
    },
    /// Delete a comment from an issue by its ID
    ///
    /// Always prints its full result regardless of --select — a small, synthesized
    /// confirmation object.
    #[command(after_help = "Example: jira issue comment remove KAN-4 10012\n\nThe comment ID is the \"id\" field in the JSON returned by comment add or issue get.")]
    Remove {
        /// Issue key, e.g. PROJ-123
        key: String,
        /// Comment ID to delete
        id: String,
    },
}

#[cfg(test)]
#[path = "tests/cli_tests.rs"]
mod tests;
