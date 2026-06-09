use clap::{Parser, Subcommand};

/// Jira CLI for LLM agents — query Jira issues from the command line.
#[derive(Debug, Parser)]
#[command(name = "jira", version, about)]
pub struct Cli {
    /// Comma-separated dot-notation paths to project from the JSON output (client-side).
    /// If omitted, the full response from Jira is printed.
    /// Example: --select summary,status.name,assignee.displayName
    /// Example: --select transitions.id,transitions.name
    #[arg(long, global = true, value_name = "PATHS")]
    pub select: Option<String>,

    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
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
    Login,
    /// Print the currently authenticated user as JSON
    Whoami,
}

#[derive(Debug, Subcommand)]
pub enum IssueCommand {
    /// Fetch a single issue by key (e.g. PROJ-123) and print it as JSON
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
    Transitions {
        /// Issue key, e.g. PROJ-123
        key: String,
    },
    /// Search issues using JQL (Jira Query Language) and return matching issues as JSON
    #[command(after_help = "Examples:\n  jira issue search --jql \"project=KAN AND status=\\\"In Progress\\\"\"\n  jira issue search --jql \"assignee=currentUser() ORDER BY created DESC\" --max-results 10\n  jira issue search --jql \"project=KAN\" --fields summary,status,priority\n\nPagination: the response includes a nextPageToken field when more results exist.\nPass its value to --page-token on the next call to fetch the following page.")]
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
    },
    /// Create a new issue in a Jira project
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
    #[command(after_help = "Example: jira issue comment add KAN-4 --body \"Blocked by network issue, retrying tomorrow\"")]
    Add {
        /// Issue key, e.g. PROJ-123
        key: String,
        /// Comment text (plain text; the CLI converts it to Jira's document format)
        #[arg(long)]
        body: String,
    },
    /// Delete a comment from an issue by its ID
    #[command(after_help = "Example: jira issue comment remove KAN-4 10012\n\nThe comment ID is the \"id\" field in the JSON returned by comment add or issue get.")]
    Remove {
        /// Issue key, e.g. PROJ-123
        key: String,
        /// Comment ID to delete
        id: String,
    },
}

#[cfg(test)]
#[path = "cli_tests.rs"]
mod tests;
