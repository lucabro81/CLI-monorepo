use clap::{Parser, Subcommand};

/// Jira CLI for LLM agents — query Jira issues from the command line.
#[derive(Debug, Parser)]
#[command(name = "jira", version, about)]
pub struct Cli {
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
}

#[cfg(test)]
#[path = "cli_tests.rs"]
mod tests;
