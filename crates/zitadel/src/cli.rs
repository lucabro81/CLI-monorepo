//! CLI surface definition — all clap structs and enums. No logic.
//!
//! Every flag uses `#[arg(long)]` only; no short aliases.

use clap::{Parser, Subcommand};

/// ZITADEL CLI for LLM agents — manage users, organizations and projects of a ZITADEL instance.
#[derive(Debug, Parser)]
#[command(name = "zitadel", version, about)]
pub struct Cli {
    /// Comma-separated dot-notation paths to project from the JSON output (client-side).
    /// Required on list/search commands: if both this and --select-all are omitted, the
    /// command fails with an error reporting the byte size of the full response and its
    /// top-level field names, so you can retry with an informed --select. Commands whose
    /// output is small and fixed-shape (e.g. auth whoami) print in full regardless — see
    /// that command's own --help.
    /// Example: --select user.id,user.userName
    #[arg(long, global = true, value_name = "PATHS", conflicts_with = "select_all")]
    pub select: Option<String>,

    /// Explicitly print the full, unfiltered JSON response instead of specifying --select.
    /// Still refused if the response exceeds a fixed byte cap (currently 30000 bytes) — the
    /// error reports the actual size and top-level fields so you can retry with --select.
    #[arg(long, global = true, conflicts_with = "select")]
    pub select_all: bool,

    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Configure the CLI for a ZITADEL instance, log in, and verify with doctor
    ///
    /// Writes app.json (instance URL, service user key, optional Native app client
    /// id; file mode 0600 because it holds the private key), logs in as the service
    /// user if one is configured, then prints the doctor JSON report and exits
    /// non-zero if any check fails. Re-running merges with the existing app.json:
    /// flags you omit keep their current value, so e.g. a Native app client id can
    /// be added later with --client-id alone. No interactive prompts.
    #[command(after_help = "Examples:\n  zitadel init --instance-url https://acme.zitadel.cloud --key-file ~/Downloads/123456789.json\n  zitadel init --client-id 123456789@zitadel-cli   # add the Native app for auth login --user later")]
    Init {
        /// Instance base URL, e.g. `https://acme.zitadel.cloud` or your self-hosted domain.
        /// Required on the first run.
        #[arg(long)]
        instance_url: Option<String>,
        /// Path to the service user's JSON key file downloaded from the console
        /// (Users > Service Users > <user> > Keys > New, type JSON). Its content is
        /// copied into app.json; the file itself is no longer needed afterwards.
        #[arg(long, value_name = "PATH")]
        key_file: Option<std::path::PathBuf>,
        /// Client id of a Native application (PKCE), needed only for auth login --user.
        #[arg(long)]
        client_id: Option<String>,
    },
    /// Check configuration, credentials, API reachability and the identity's roles
    ///
    /// Runs four checks in order and prints a JSON report with a status field
    /// ("ok", "error" or "skipped") per check: `app_config` (app.json), `credentials`
    /// (stored token, renewed if expiring; identity is `service_user` or `user`),
    /// api (GET /auth/v1/users/me) and memberships (the administrator roles of the
    /// identity per instance/organization/project — these decide which commands
    /// will succeed). Later checks are skipped when an earlier one fails. Exits
    /// non-zero unless every check is ok. Always prints the full report regardless
    /// of --select (an explicit --select is still honored).
    #[command(after_help = "Examples:\n  zitadel doctor\n  zitadel doctor --select memberships")]
    Doctor,
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
    /// Show the identity the CLI is authenticated as
    ///
    /// Prints the ZITADEL user behind the stored credentials (GET /auth/v1/users/me):
    /// id, userName, loginNames, its organization (details.resourceOwner) and whether
    /// it is a service user ("machine") or a human ("human"). Always prints the full
    /// response regardless of --select — it is a single small object. An explicit
    /// --select is still honored.
    #[command(after_help = "Examples:\n  zitadel auth whoami\n  zitadel auth whoami --select user.id,user.userName,user.details.resourceOwner")]
    Whoami,
}

#[cfg(test)]
#[path = "tests/cli_tests.rs"]
mod tests;
