//! CLI surface definition — all clap structs and enums. No logic.
//!
//! Every flag uses `#[arg(long)]` only; no short aliases.

use clap::{Parser, Subcommand};

/// ZITADEL CLI for LLM agents — manage users, organizations and projects of a ZITADEL instance.
#[derive(Debug, Parser)]
#[command(name = "zitadel", version, about)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Manage authentication with the ZITADEL instance
    Auth {
        #[command(subcommand)]
        command: AuthCommand,
    },
}

#[derive(Debug, Subcommand)]
pub enum AuthCommand {
    /// Log in and save credentials to credentials.json
    ///
    /// Logs in as the service user configured in app.json (private key JWT):
    /// signs a JWT with the service user's key and exchanges it for an access
    /// token. No browser, no human interaction. The token is renewed
    /// automatically when it expires, so this is normally only needed once.
    /// What the CLI may then do is decided by the ZITADEL administrator roles
    /// granted to the service user (e.g. `IAM_OWNER`, `ORG_OWNER`).
    #[command(after_help = "Example:\n  zitadel auth login")]
    Login,
}

#[cfg(test)]
#[path = "tests/cli_tests.rs"]
mod tests;
