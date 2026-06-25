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
    /// Interactive onboarding: create app.json, run the OAuth user login, verify with doctor
    ///
    /// Guides a human through setting up the Google OAuth 2.0 Desktop app client,
    /// writes app.json, runs the interactive (--user) login flow, then prints a
    /// doctor JSON report as confirmation. Pass --client-id and --client-secret
    /// to skip interactive prompts. Does not set up the non-interactive
    /// domain-wide-delegation flow — see README.md for that (requires a
    /// Workspace super-admin).
    #[command(after_help = "Example (interactive):\n  google-chat init\n\nExample (non-interactive):\n  google-chat init --client-id <ID> --client-secret <SECRET>")]
    Init {
        /// Google OAuth Desktop app client ID (skips interactive prompt if provided)
        #[arg(long)]
        client_id: Option<String>,
        /// Google OAuth Desktop app client secret (skips interactive prompt if provided)
        #[arg(long)]
        client_secret: Option<String>,
    },
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
    /// Work with Google Chat spaces (group chats, DMs, named spaces)
    Spaces {
        #[command(subcommand)]
        command: SpacesCommand,
    },
    /// Work with messages in a Google Chat space
    Messages {
        #[command(subcommand)]
        command: MessagesCommand,
    },
    /// Manage Workspace Events subscriptions that push Chat events to Pub/Sub
    Subscription {
        #[command(subcommand)]
        command: SubscriptionCommand,
    },
    /// Stream messages from a Pub/Sub subscription as they arrive
    ///
    /// Opens a streaming pull on the given Pub/Sub subscription and prints
    /// each received message as one JSON line (NDJSON) to stdout, then
    /// acknowledges it. Runs until interrupted (Ctrl+C / SIGINT, or SIGTERM —
    /// the signal sent by `kill <pid>`/`pkill`, the expected way for an
    /// agent or script controlling the process to stop it). Refreshes its
    /// own access token in the background as needed, and periodically
    /// renews the Workspace Events subscription's TTL (it otherwise expires
    /// after ~4h), so it can run indefinitely without being restarted.
    /// Prints its PID to stderr at startup so the caller has a handle to
    /// stop it later.
    #[command(after_help = "Example:\n  google-chat listen --pubsub-subscription projects/my-project/subscriptions/my-sub --workspace-events-subscription subscriptions/chat-spaces-abc123\n\n--workspace-events-subscription is the \"name\" field from `subscription create`'s output.\nStop it with Ctrl+C (foreground) or `kill <pid>`/`pkill -f \"google-chat listen\"` (background).\nPass --max-messages to exit automatically after receiving N messages (useful for smoke tests).")]
    Listen {
        /// Full Pub/Sub subscription resource name: "projects/{project}/subscriptions/{subscription}"
        #[arg(long)]
        pubsub_subscription: String,
        /// Workspace Events subscription to keep renewed, from the "name" field of `subscription create`'s output: "subscriptions/{id}"
        #[arg(long)]
        workspace_events_subscription: String,
        /// Exit automatically after receiving this many messages, instead of running until interrupted
        #[arg(long)]
        max_messages: Option<u32>,
    },
}

#[derive(Debug, Subcommand)]
pub enum SpacesCommand {
    /// List spaces the authenticated identity belongs to, as JSON
    ///
    /// Returns {"spaces": [...], "nextPageToken": "..."}. Pass --page-token
    /// (taken from a previous response's nextPageToken) to fetch the next page.
    #[command(after_help = "Examples:\n  google-chat spaces list\n  google-chat spaces list --page-size 20\n  google-chat spaces list --page-token <TOKEN>\n  google-chat spaces list --select spaces.name,spaces.displayName,spaces.spaceType")]
    List {
        /// Maximum number of spaces to return (default: 100; the server may return fewer)
        #[arg(long, default_value = "100")]
        page_size: u32,
        /// Cursor token for the next page, from the nextPageToken field of a previous response
        #[arg(long)]
        page_token: Option<String>,
    },
}

#[derive(Debug, Subcommand)]
pub enum MessagesCommand {
    /// List messages in a space, as JSON
    ///
    /// Returns {"messages": [...], "nextPageToken": "..."}. Defaults to
    /// chronological order (createTime ASC, the Chat API's own default) — this
    /// is what makes it usable to recover conversation context after a gap or
    /// aggressive history summarization: page forward through --page-token to
    /// walk the full history. Pass --order-by "createTime DESC" instead to get
    /// the most recent messages first.
    #[command(after_help = "Examples:\n  google-chat messages list --space spaces/AAQA-_d58OQ\n  google-chat messages list --space AAQA-_d58OQ --page-size 20\n  google-chat messages list --space AAQA-_d58OQ --order-by \"createTime DESC\"\n  google-chat messages list --space AAQA-_d58OQ --select messages.text,messages.sender.displayName,messages.createTime\n\n--space accepts either the bare id or the full \"spaces/...\" resource name\n(as printed in the \"name\" field of `spaces list` output).")]
    List {
        /// Space to list messages from — bare id or full "spaces/{id}" resource name
        #[arg(long)]
        space: String,
        /// Maximum number of messages to return (default: 100; the server may return fewer)
        #[arg(long, default_value = "100")]
        page_size: u32,
        /// Cursor token for the next page, from the nextPageToken field of a previous response
        #[arg(long)]
        page_token: Option<String>,
        /// Ordering of returned messages, e.g. "createTime ASC" (default) or "createTime DESC"
        #[arg(long)]
        order_by: Option<String>,
    },
    /// Send a plain-text message to a space and print the created Message as JSON
    ///
    /// Creates real, visible state in the target space — the message appears
    /// to everyone in it immediately. Prints the created Message resource,
    /// including its "name" field (needed to identify it in future calls).
    #[command(after_help = "Example:\n  google-chat messages send --space spaces/AAQA-_d58OQ --text \"Status update: deploy complete\"\n\n--space accepts either the bare id or the full \"spaces/...\" resource name\n(as printed in the \"name\" field of `spaces list` output).")]
    Send {
        /// Space to send the message to — bare id or full "spaces/{id}" resource name
        #[arg(long)]
        space: String,
        /// Plain-text message body
        #[arg(long)]
        text: String,
    },
}

#[derive(Debug, Subcommand)]
pub enum SubscriptionCommand {
    /// Create a Workspace Events subscription that pushes Chat events for a space to a Pub/Sub topic
    ///
    /// Ensures a pull subscription exists on the given Pub/Sub topic (creates
    /// one if missing; does nothing if it already exists — safe to call
    /// without checking first), then registers a Workspace Events
    /// subscription targeting the space, delivering matching events to that
    /// topic. Prints the created Workspace Events subscription resource as
    /// JSON. Pair with `google-chat listen --pubsub-subscription <name>` to
    /// receive the events.
    #[command(after_help = "Example:\n  google-chat subscription create --space spaces/AAQA-_d58OQ --topic projects/my-project/topics/my-topic --pubsub-subscription projects/my-project/subscriptions/my-sub\n\n--space accepts either the bare id or the full \"spaces/...\" resource name.\n--event-type can be repeated; defaults to google.workspace.chat.message.v1.created.\nValid event types: google.workspace.chat.message.v1.created, .updated, .deleted.")]
    Create {
        /// Space to subscribe to — bare id or full "spaces/{id}" resource name
        #[arg(long)]
        space: String,
        /// Pub/Sub topic that will receive events: "projects/{project}/topics/{topic}"
        #[arg(long)]
        topic: String,
        /// Pull subscription on that topic, created if it does not already exist: "projects/{project}/subscriptions/{subscription}"
        #[arg(long)]
        pubsub_subscription: String,
        /// Chat event type to subscribe to (repeatable); default: google.workspace.chat.message.v1.created
        #[arg(long, default_values_t = ["google.workspace.chat.message.v1.created".to_string()])]
        event_type: Vec<String>,
    },
    /// Delete a Workspace Events subscription so it stops delivering events
    ///
    /// Call this when an agent is done with a conversation, to stop
    /// receiving its events immediately rather than waiting for the
    /// subscription to expire on its own (~4h, or never, if something is
    /// still calling `listen` and renewing it). Tightens access to exactly
    /// the conversations currently in progress instead of leaving stale
    /// subscriptions live.
    #[command(after_help = "Example:\n  google-chat subscription delete --name subscriptions/chat-spaces-abc123\n\n--name is the \"name\" field from `subscription create`'s output.")]
    Delete {
        /// Workspace Events subscription to delete: "subscriptions/{id}"
        #[arg(long)]
        name: String,
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
