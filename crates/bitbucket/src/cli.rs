//! CLI surface definition — all clap structs and enums.
//!
//! No logic lives here — this file is purely argument parsing and help text.
//! Every flag uses `#[arg(long)]` only; no short aliases.

use clap::{Parser, Subcommand};

/// Bitbucket CLI for LLM agents — query Bitbucket Cloud from the command line.
#[derive(Debug, Parser)]
#[command(name = "bitbucket", version, about)]
pub struct Cli {
    /// Comma-separated dot-notation paths to project from the JSON output (client-side).
    /// If omitted, the full response from Bitbucket is printed.
    /// Example: --select `uuid,display_name`
    #[arg(long, global = true, value_name = "PATHS")]
    pub select: Option<String>,

    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Manage authentication with Bitbucket
    Auth {
        #[command(subcommand)]
        command: AuthCommand,
    },
}

#[derive(Debug, Subcommand)]
pub enum AuthCommand {
    /// Run the OAuth 2.0 `client_credentials` flow and store credentials locally
    ///
    /// Exchanges the OAuth consumer's `client_id`/`client_secret` (from app.json) for
    /// an access token via HTTP Basic auth. No browser, no user interaction. The
    /// token has no `refresh_token` — it is renewed automatically by re-running the
    /// same exchange when expired.
    ///
    /// Run this once per machine; tokens are renewed automatically after that.
    #[command(after_help = "Example:\n  bitbucket auth login\n\nRequires app.json to exist at ~/.config/bitbucket-cli/app.json with the OAuth\nconsumer's Key/Secret: {\"client_id\": \"...\", \"client_secret\": \"...\"}")]
    Login,
    /// Print the currently authenticated account as JSON
    #[command(after_help = "Examples:\n  bitbucket auth whoami\n  bitbucket auth whoami --select uuid,display_name")]
    Whoami,
}

#[cfg(test)]
#[path = "cli_tests.rs"]
mod tests;
