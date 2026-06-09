use clap::{Parser, Subcommand};

/// Jira CLI for LLM agents — query Jira issues from the command line.
#[derive(Debug, Parser)]
#[command(name = "jira", version, about)]
pub struct Cli {
    /// Comma-separated dot-notation paths to include in the JSON output.
    /// If omitted, the full response from Jira is printed.
    /// Example: --fields summary,status.name,assignee.displayName
    /// Example: --fields transitions.id,transitions.name
    #[arg(long, global = true, value_name = "PATHS")]
    pub fields: Option<String>,

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
