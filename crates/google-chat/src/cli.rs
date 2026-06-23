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

/// Google Chat CLI for LLM agents — read and send Google Chat messages from the command line.
#[derive(Debug, Parser)]
#[command(name = "google-chat", version, about)]
pub struct Cli {
    /// Comma-separated dot-notation paths to project from the JSON output (client-side).
    /// If omitted, the full response from the Chat API is printed.
    /// Example: --select spaces.name,spaces.displayName
    #[arg(long, global = true, value_name = "PATHS")]
    pub select: Option<String>,

    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Check that the CLI is correctly configured and can reach the Google Chat API
    ///
    /// Runs three checks in order: app credentials file, stored OAuth tokens,
    /// and a live API call (spaces.list). Prints a JSON object with a status
    /// field per check. Exits non-zero if any check fails or is skipped.
    #[command(after_help = "Examples:\n  google-chat doctor\n  google-chat doctor --select app_config.status,credentials.status,api.status\n\nEach check has a status field: \"ok\", \"error\", or \"skipped\".\nLater checks are skipped if an earlier one fails.")]
    Doctor,
    /// Manage authentication with Google Chat
    Auth {
        #[command(subcommand)]
        command: AuthCommand,
    },
}

#[derive(Debug, Subcommand)]
pub enum AuthCommand {
    /// Run the OAuth 2.0 login flow and store credentials locally
    ///
    /// By default runs the non-interactive domain-wide-delegation flow: signs a
    /// JWT assertion with the service account's private key, impersonating the
    /// configured Workspace user (no browser, no human interaction). This is the
    /// expected mode for agent-driven usage.
    ///
    /// Pass --user for the interactive OAuth 2.0 Authorization Code + PKCE flow:
    /// opens the browser for consent, receives the callback on localhost:8080,
    /// exchanges the code for tokens, and stores a `refresh_token` for automatic
    /// renewal.
    ///
    /// Run this once per machine; tokens are renewed automatically after that.
    #[command(after_help = "Examples:\n  google-chat auth login              # service account (domain-wide delegation)\n  google-chat auth login --user       # human account (OAuth 2.0 + PKCE)\n\nRequires app.json to exist at ~/.config/google-chat-cli/app.json.\nRun `google-chat init` first if you have not set up the OAuth app yet.")]
    Login {
        /// Use the interactive OAuth 2.0 Authorization Code + PKCE flow for a human Google account
        #[arg(long)]
        user: bool,
    },
}

#[cfg(test)]
#[path = "tests/cli_tests.rs"]
mod tests;
