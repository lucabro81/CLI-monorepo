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
    /// issue create/delete/assign/transitions/transition/comment add/comment remove) are
    /// exempt and print in full regardless — see that command's own --help.
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

    /// Act as the human who logged in with `jira auth login --user` instead of the
    /// Service Account. Without it every command acts as the Service Account
    /// (`jira auth login`). Both identities are stored side by side and renewed
    /// automatically, so switching between them needs no new login. On `init`,
    /// `auth login`, `auth whoami` and `doctor` it selects which identity to set
    /// up, log in or check.
    #[arg(long, global = true)]
    pub user: bool,

    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Onboarding: save an identity's OAuth app to app.json, log in, verify with doctor
    ///
    /// Without --user: sets up the Service Account (admin.atlassian.com credential),
    /// writes app.json's "service" section, runs the non-interactive login. With --user:
    /// sets up the 3LO app (developer console) for the human identity, writes the "user"
    /// section, runs the browser login. The other section of app.json is left untouched.
    /// Then prints a doctor JSON report for that identity. Pass --client-id and
    /// --client-secret to skip interactive prompts.
    #[command(after_help = "Examples:\n  jira init --client-id <ID> --client-secret <SECRET>          # Service Account\n  jira init --user --client-id <ID> --client-secret <SECRET>   # 3LO app, human login in the browser\n  jira init                                                    # interactive prompts")]
    Init {
        /// OAuth client ID of the identity being set up (skips interactive prompt if provided)
        #[arg(long)]
        client_id: Option<String>,
        /// OAuth client secret of the identity being set up (skips interactive prompt if provided)
        #[arg(long)]
        client_secret: Option<String>,
    },
    /// Check that the CLI is correctly configured and can reach the Jira API
    ///
    /// Runs checks in order: app credentials file, stored OAuth tokens, a live
    /// API call, granted OAuth scopes, global and per-project permissions.
    /// Prints a JSON object with a status field per check. Exits non-zero if any
    /// check fails or is skipped. Also reports `pending_login` (a two-step
    /// `auth login --user --remote` waiting for its code): informational, never
    /// counted in the exit code. Checks the Service Account, or the human with
    /// --user; `identities` (informational) shows which of the two are logged in
    /// and flags a leftover pre-#164 credentials.json. Always prints its full
    /// result regardless of --select — the report is generated internally and is
    /// always small and fixed-shape.
    #[command(after_help = "Examples:\n  jira doctor\n  jira doctor --user\n  jira doctor --select app_config.status,credentials.status,api.status\n\nEach check has a status field: \"ok\", \"error\", or \"skipped\".\nLater checks are skipped if an earlier one fails.")]
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
    /// Work with Jira users
    User {
        #[command(subcommand)]
        command: UserCommand,
    },
    /// Work with Jira projects
    Project {
        #[command(subcommand)]
        command: ProjectCommand,
    },
}

#[derive(Debug, Subcommand)]
pub enum AuthCommand {
    /// Run the OAuth 2.0 login flow and store credentials locally
    ///
    /// By default runs the `client_credentials` flow for the Service Account: no
    /// browser, no user interaction — the access token is exchanged directly
    /// from app.json's "service" section. Saved to credentials-service.json.
    ///
    /// Pass --user for the interactive OAuth 2.0 (3LO) + PKCE flow for a human
    /// Atlassian account, with app.json's "user" section (a 3LO app): opens the
    /// browser for consent, receives the callback on localhost:8080, exchanges the
    /// code for tokens, and stores a `refresh_token` for automatic renewal in
    /// credentials-user.json. Each login replaces only its own identity's
    /// credentials: afterwards every command acts as the Service Account, or as
    /// the human when it is given --user.
    ///
    /// With --user --remote: a two-step login for a person who is not at this
    /// machine. Step 1 (--remote --redirect-uri) opens no browser and listens on
    /// no port: it prints JSON `{authorize_url, state, expires_at}` for the caller
    /// to hand to the person. Atlassian then redirects the person to
    /// --redirect-uri (it must be one of the 3LO app's callback URLs) with `code`
    /// and `state`. Step 2 (--code --state) exchanges the code, saves the
    /// credentials, then prints what `auth whoami --user` prints. The pending login
    /// expires after 10 minutes, its state is single-use, and it lives in this
    /// config folder (`XDG_CONFIG_HOME`). Uses the "user" section (a 3LO app).
    ///
    /// Run this once per machine; tokens are renewed automatically after that.
    #[command(after_help = "Examples:\n  jira auth login              # service account (client_credentials)\n  jira auth login --user       # human account (OAuth 2.0 3LO + PKCE)\n  jira auth login --user --remote --redirect-uri https://app.example.com/oauth/callback   # step 1\n  jira auth login --user --code <CODE> --state <STATE>                                   # step 2\n\nRequires app.json at ~/.config/jira-cli/app.json with the identity's section.\nRun `jira init` (or `jira init --user`) first if it is missing.")]
    Login {
        /// Step 1 of a two-step login for someone not at this machine: print the authorize URL instead of opening a browser
        // "needs --user" is checked in LoginMode::from_flags: clap's `requires`
        // cannot see a global --user written before the subcommand.
        #[arg(long, requires = "redirect_uri", conflicts_with_all = ["code", "state"])]
        remote: bool,
        /// With --remote: where Atlassian sends the person back; must be a callback URL of the 3LO app
        #[arg(long, requires = "remote")]
        redirect_uri: Option<String>,
        /// Step 2: the `code` query parameter Atlassian appended to the redirect URI
        #[arg(long, requires = "state")]
        code: Option<String>,
        /// Step 2: the `state` query parameter Atlassian appended to the redirect URI
        #[arg(long, requires = "code")]
        state: Option<String>,
    },
    /// Print the account the CLI acts as, as JSON: the Service Account, or the human with --user
    ///
    /// Always prints its full result regardless of --select — an identity check,
    /// small and fixed-shape.
    #[command(after_help = "Examples:\n  jira auth whoami\n  jira auth whoami --user\n  jira auth whoami --select displayName,emailAddress,accountId")]
    Whoami,
}

#[derive(Debug, Subcommand)]
pub enum IssueCommand {
    /// Fetch a single issue by key (e.g. PROJ-123) and print it as JSON
    #[command(after_help = "Examples:\n  jira issue get PROJ-123 --select key,fields.summary,fields.status.name\n  jira issue get PROJ-123 --select fields.summary,fields.status.name,fields.assignee.displayName,fields.priority.name")]
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
    #[command(after_help = "Examples:\n  jira issue search --jql \"project=KAN AND status=\\\"In Progress\\\"\" --select issues.key,issues.fields.summary\n  jira issue search --jql \"assignee=5b10ac8d82e05b22cc7d4ef5 ORDER BY created DESC\" --max-results 10 --select issues.key,issues.fields.summary,nextPageToken\n  jira issue search --jql \"project=KAN\" --fields summary,status,priority --select issues.key,issues.fields.summary,issues.fields.status.name,issues.fields.priority.name\n  jira issue search --jql \"project=KAN AND status!=Done\" --stale-days 14 --select issues.key,issues.fields.summary,issues.fields.updated\n  jira issue search --jql \"project=KAN\" --select issues.fields.summary,issues.fields.status.name\n\nPagination: the response includes a nextPageToken field when more results exist.\nPass its value to --page-token on the next call to fetch the following page.\n\nTo filter by a person, use their account ID (find it with `jira user search --query <name>`),\nnot currentUser(): that resolves to the account the CLI acts as (the service account, or\nthe human with --user), not necessarily the person you are working for.\n\n--stale-days N adds \"AND updated <= -Nd\" to --jql (inserted before ORDER BY, if present) to\nfind issues that have not been updated in at least N days.")]
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
    #[command(after_help = "Examples:\n  jira issue create --project KAN --type Task --summary \"Fix login bug\"\n  jira issue create --project KAN --type Bug --summary \"Crash on startup\" --description \"Happens on macOS 14\" --priority High\n  jira issue create --project KAN --type Task --summary \"Add caching\" --description \"## Plan\\n\\n- profile the endpoint\\n- add a cache layer\\n\\nSee \\`get_data()\\`.\"\n  jira issue create --project KAN --type Subtask --summary \"Fix typo\" --parent KAN-10")]
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
        /// Optional description (Markdown; converted to Jira document format —
        /// headings, bullet/numbered lists, inline/fenced code, bold, italic,
        /// links, and line breaks are all supported)
        #[arg(long)]
        description: Option<String>,
        /// Optional assignee account ID (use `auth whoami` to get your own)
        #[arg(long)]
        assignee: Option<String>,
        /// Optional priority name, e.g. High, Medium, Low
        #[arg(long)]
        priority: Option<String>,
        /// Optional parent issue key, e.g. KAN-10. Sets this issue as a subtask
        /// of the given issue (any project), or as a child of an Epic on
        /// team-managed projects. Not supported for Epic linkage on
        /// company-managed projects, which uses a separate custom field instead.
        #[arg(long)]
        parent: Option<String>,
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
    /// Assign or unassign an issue
    ///
    /// Always prints its full result regardless of --select — a small, synthesized
    /// confirmation object.
    #[command(after_help = "Examples:\n  jira issue assign KAN-4 --assignee 5b10ac8d82e05b22cc7d4ef5\n  jira issue assign KAN-4 --unassign\n\nExactly one of --assignee or --unassign must be passed.\nUse `jira user search --query <name>` to find an account ID.")]
    Assign {
        /// Issue key, e.g. PROJ-123
        key: String,
        /// Account ID of the user to assign. Find one with `jira user search --query <name>`.
        #[arg(long, conflicts_with = "unassign")]
        assignee: Option<String>,
        /// Remove the current assignee instead of assigning a new one
        #[arg(long)]
        unassign: bool,
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
    ///
    /// Two ways to tag/mention a user in the comment, which can be combined:
    /// --mention tags a user at the start of the comment; embedding
    /// `{{mention:ACCOUNT_ID}}` anywhere inside --body tags a user at that exact
    /// position in the text. Use `jira user search --query <name>` first to find
    /// the account ID to mention.
    #[command(after_help = "Examples:\n  jira issue comment add KAN-4 --body \"Blocked by network issue, retrying tomorrow\"\n  jira issue comment add KAN-4 --mention 5b10ac8d82e05b22cc7d4ef5 --body \"can you take a look?\"\n  jira issue comment add KAN-4 --body \"Thanks {{mention:5b10ac8d82e05b22cc7d4ef5}} for the fix\"\n  jira issue comment add KAN-4 --body \"Root cause:\\n\\n- stale cache\\n- missing invalidation on \\`update()\\`\"")]
    Add {
        /// Issue key, e.g. PROJ-123
        key: String,
        /// Comment text (Markdown; converted to Jira's document format — headings,
        /// bullet/numbered lists, inline/fenced code, bold, italic, links, and line
        /// breaks are all supported). Embed `{{mention:ACCOUNT_ID}}` anywhere in
        /// this text to tag a user at that exact position, in addition to or
        /// instead of --mention.
        #[arg(long)]
        body: String,
        /// Account ID of a user to tag/mention at the start of the comment.
        /// Find an account ID with `jira user search --query <name>`.
        #[arg(long)]
        mention: Option<String>,
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

#[derive(Debug, Subcommand)]
pub enum UserCommand {
    /// Search for Jira users by name or email fragment and print matches as JSON
    ///
    /// Requires the "Browse users and groups" global permission. Without it, Jira
    /// does not return an error — it silently returns an empty match list. Check
    /// `jira doctor`'s permissions report if searches unexpectedly return nothing.
    #[command(after_help = "Examples:\n  jira user search --query \"Jane Doe\" --select accountId,displayName\n  jira user search --query jane.doe@example.com --select accountId,displayName,emailAddress\n\nUse the accountId from the result as the ID for --mention on `issue comment add`.")]
    Search {
        /// Name or email fragment to search for
        #[arg(long)]
        query: String,
    },
}

#[derive(Debug, Subcommand)]
pub enum ProjectCommand {
    /// Search for Jira projects by name or key fragment and print matches as JSON
    ///
    /// `--query` is a literal substring/prefix filter (case-insensitive) against
    /// both the project key and name — not a query language. Use this to find a
    /// project's key when you only know (part of) its name.
    #[command(after_help = "Examples:\n  jira project search --query Mercury --select values.key,values.name\n  jira project search --query mercur --select values.key,values.name\n\nUse the key from the result as the --project value for `issue create` or in JQL.")]
    Search {
        /// Name or key fragment to search for
        #[arg(long)]
        query: String,
    },
}

#[cfg(test)]
#[path = "tests/cli_tests.rs"]
mod tests;
