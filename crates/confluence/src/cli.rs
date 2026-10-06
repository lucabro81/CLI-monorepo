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
use oauth_user_login::UserId;

/// Confluence Cloud CLI for LLM agents — read and write Confluence pages from the command line.
#[derive(Debug, Parser)]
#[command(name = "confluence", version, about)]
pub struct Cli {
    /// Comma-separated dot-notation paths to project from the JSON output (client-side).
    /// Required on most commands: if both this and --select-all are omitted, the
    /// command fails with an error reporting the byte size of the full response and
    /// its top-level field names, so you can retry with an informed --select. A few
    /// commands whose output is always small and fixed-shape (doctor, auth whoami, auth logout)
    /// are exempt and print in full regardless — see that command's own --help.
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

    /// Act as the person with this id instead of the Service Account: the human
    /// who logged in with `confluence auth login --user <USER_ID>`. The id is your own
    /// name for that person, a lowercase slug (a-z, 0-9, '.', '_', '-', e.g.
    /// jane.doe). Without it every command acts as the Service Account
    /// (`confluence auth login`). The Service Account and every person are stored side
    /// by side and renewed automatically, so switching between them needs no new
    /// login. On `init`, `auth login`, `auth whoami`, `auth logout` and `doctor` it
    /// selects which identity to set up, log in, check or remove.
    #[arg(long, global = true, value_name = "USER_ID", value_parser = UserId::parse)]
    pub user: Option<UserId>,

    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Onboarding: save an identity's OAuth app to app.json, log in, verify with doctor
    ///
    /// Without --user: sets up the Service Account (admin.atlassian.com credential),
    /// writes app.json's "service" section, runs the non-interactive login. With
    /// `--user <USER_ID>`: sets up the 3LO app (developer console) that every person
    /// logs in with, writes the "user" section, and logs that person in through the
    /// browser. The other section of app.json is left untouched. Then prints a doctor
    /// JSON report for that identity. Pass --client-id and --client-secret to skip
    /// interactive prompts. To log in more people once the 3LO app is set up, use
    /// `confluence auth login --user <USER_ID>`.
    #[command(after_help = "Examples:\n  confluence init --client-id <ID> --client-secret <SECRET>          # Service Account\n  confluence init --user jane.doe --client-id <ID> --client-secret <SECRET>   # 3LO app, jane.doe logs in in the browser\n  confluence init                                                    # interactive prompts")]
    Init {
        /// OAuth client ID of the identity being set up (skips interactive prompt if provided)
        #[arg(long)]
        client_id: Option<String>,
        /// OAuth client secret of the identity being set up (skips interactive prompt if provided)
        #[arg(long)]
        client_secret: Option<String>,
    },
    /// Check that the CLI is correctly configured and can reach the Confluence API
    ///
    /// Runs four checks in order: app credentials file, stored OAuth tokens, a
    /// live API call, and the OAuth scopes granted to the token. Prints a JSON
    /// object with a status field per check. Exits non-zero if any check fails
    /// or is skipped. Also reports `pending_login` (a two-step
    /// `auth login --user <USER_ID> --remote` waiting for its code): informational,
    /// never counted in the exit code. Checks the Service Account, or the person
    /// with `--user <USER_ID>`; `identities` (informational) shows whether the
    /// Service Account is logged in, lists the ids of the people logged in, and
    /// flags credentials files of earlier layouts (credentials.json,
    /// credentials-user.json), no longer read. Always prints its full result
    /// regardless of --select — the report is generated internally and is always
    /// small and fixed-shape.
    #[command(after_help = "Examples:\n  confluence doctor\n  confluence doctor --user jane.doe\n  confluence doctor --select app_config.status,credentials.status,api.status\n\nEach check has a status field: \"ok\", \"error\", or \"skipped\".\nLater checks are skipped if an earlier one fails.")]
    Doctor,
    /// Manage authentication with Confluence
    Auth {
        #[command(subcommand)]
        command: AuthCommand,
    },
    /// Work with Confluence pages
    Page {
        #[command(subcommand)]
        command: PageCommand,
    },
    /// Work with Confluence spaces
    Space {
        #[command(subcommand)]
        command: SpaceCommand,
    },
    /// Work with Confluence content templates
    Template {
        #[command(subcommand)]
        command: TemplateCommand,
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
    /// Pass `--user <USER_ID>` for the interactive OAuth 2.0 (3LO) + PKCE flow for a
    /// person's Atlassian account, with app.json's "user" section (a 3LO app shared
    /// by every person): opens the browser for consent, receives the callback on
    /// localhost:8080, exchanges the code for tokens, and stores a `refresh_token`
    /// for automatic renewal in `users/<USER_ID>/credentials.json`. Each login
    /// replaces only its own identity's credentials: afterwards every command acts
    /// as the Service Account, or as that person when it is given `--user <USER_ID>`.
    ///
    /// With `--user <USER_ID> --remote`: a two-step login for a person who is not at this
    /// machine. Step 1 (--remote --redirect-uri) opens no browser and listens on
    /// no port: it prints JSON `{authorize_url, state, expires_at}` for the caller
    /// to hand to the person. Atlassian then redirects the person to
    /// --redirect-uri (it must be one of the 3LO app's callback URLs) with `code`
    /// and `state`. Step 2 (--code --state) exchanges the code, saves the
    /// credentials, then prints what `auth whoami --user <USER_ID>` prints. The
    /// pending login expires after 10 minutes, its state is single-use, and it lives
    /// in that person's folder of this config folder (`XDG_CONFIG_HOME`), so several
    /// people can be mid-login at once. Uses the "user" section (a 3LO app).
    ///
    /// Run this once per identity per machine; tokens are renewed automatically
    /// after that (one renewal at a time per identity, so parallel calls are safe).
    #[command(after_help = "Examples:\n  confluence auth login              # service account (client_credentials)\n  confluence auth login --user jane.doe       # a person (OAuth 2.0 3LO + PKCE)\n  confluence auth login --user jane.doe --remote --redirect-uri https://app.example.com/oauth/callback   # step 1\n  confluence auth login --user jane.doe --code <CODE> --state <STATE>                                   # step 2\n\nRequires app.json at ~/.config/confluence-cli/app.json with the identity's section.\nRun `confluence init` (or `confluence init --user <USER_ID>`) first if it is missing.")]
    Login {
        /// Step 1 of a two-step login for someone not at this machine: print the authorize URL instead of opening a browser
        // "needs --user <USER_ID>" is checked in LoginMode::from_flags: clap's
        // `requires` cannot see a global --user written before the subcommand.
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
    /// Print the account the CLI acts as, as JSON: the Service Account, or the person with `--user <USER_ID>`
    ///
    /// Always prints its full result regardless of --select — an identity check,
    /// small and fixed-shape.
    #[command(after_help = "Examples:\n  confluence auth whoami\n  confluence auth whoami --user jane.doe\n  confluence auth whoami --select displayName,email,accountId")]
    Whoami,
    /// Remove the stored login of the Service Account, or of the person with `--user <USER_ID>`
    ///
    /// Deletes that identity's credentials (for a person, their whole folder,
    /// including a pending remote login) from this machine, so commands can no
    /// longer act as it until it logs in again. Other identities are untouched.
    /// app.json is kept. Local only: the tokens are not revoked at Atlassian
    /// (a person can revoke the app's access from their Atlassian account).
    /// Prints JSON `{"logged_out": "service"}` or `{"logged_out": "user:<USER_ID>"}`;
    /// fails if that identity had no stored login.
    #[command(after_help = "Examples:\n  confluence auth logout --user jane.doe\n  confluence auth logout")]
    Logout,
}

#[derive(Debug, Subcommand)]
pub enum PageCommand {
    /// Fetch a single page by ID, including its body, and print it as JSON
    #[command(after_help = "Examples:\n  confluence page get 123456\n  confluence page get 123456 --select title,body.storage.value,version.number")]
    Get {
        /// Page ID
        id: String,
    },
    /// Create a new page in a space
    ///
    /// Exactly one of --body, --body-file, or --template-id supplies the
    /// page content. --body is raw Confluence storage-format XHTML (the same
    /// format `page get`'s body.storage.value returns) — plain text with no
    /// markup is also valid storage format. --body-file reads that same kind
    /// of content from a local file instead of a command-line argument — a
    /// convenience for longer content, unrelated to Confluence's own
    /// Template feature. --template-id copies the body of an existing
    /// Confluence content template (find one's ID via the Confluence UI:
    /// Space settings -> Content Types -> Templates) — Confluence has no API
    /// to create a page "from" a template directly, so this fetches the
    /// template's body and submits it as this page's initial content, same
    /// as duplicating it by hand.
    #[command(after_help = "Examples:\n  confluence page create --space-id 98765 --title \"Sprint Notes\" --body \"<p>Agenda</p>\"\n  confluence page create --space-id 98765 --title \"Runbook\" --body-file ./runbook-content.html\n  confluence page create --space-id 98765 --title \"Retro\" --template-id 4321 --parent-id 111222")]
    Create {
        /// Numeric ID of the space to create the page in — find one with `space list`
        #[arg(long)]
        space_id: String,
        /// Page title
        #[arg(long)]
        title: String,
        /// Optional parent page ID, to create this page as a child of another
        #[arg(long)]
        parent_id: Option<String>,
        /// Page body as raw Confluence storage-format XHTML
        #[arg(long, conflicts_with_all = ["body_file", "template_id"])]
        body: Option<String>,
        /// Path to a local file whose content becomes the page body (same
        /// format as --body, just read from a file instead of the command line)
        #[arg(long, conflicts_with_all = ["body", "template_id"])]
        body_file: Option<String>,
        /// ID of an existing Confluence content template to copy as the page body
        #[arg(long, conflicts_with_all = ["body", "body_file"])]
        template_id: Option<String>,
    },
    /// Update an existing page's title and/or body
    ///
    /// Confluence's v2 API replaces the whole page on every update — there is
    /// no partial-patch endpoint. This command fetches the page's current
    /// title, body, and version number first, then submits a full update with
    /// your --title and/or --body overriding just those fields (the other
    /// keeps its current value) and the version number incremented by one.
    #[command(after_help = "Examples:\n  confluence page update 123456 --title \"Sprint Notes (updated)\"\n  confluence page update 123456 --body \"<p>New agenda</p>\"\n\nAt least one of --title or --body is required.")]
    Update {
        /// Page ID
        id: String,
        /// New page title
        #[arg(long)]
        title: Option<String>,
        /// New page body as raw Confluence storage-format XHTML
        #[arg(long)]
        body: Option<String>,
    },
    /// Search Confluence content using CQL (Confluence Query Language) and print matches as JSON
    #[command(after_help = "Examples:\n  confluence page search --cql \"type=page AND space=ENG AND title~\\\"Runbook\\\"\"\n  confluence page search --cql \"type=page AND space=ENG\" --limit 10\n  confluence page search --cql \"type=page\" --start 25\n\nPagination: the response's size field tells you how many results this page\nreturned; pass --start <previous start + limit> to fetch the next page.")]
    Search {
        /// CQL query string, e.g. "type=page AND space=ENG"
        #[arg(long)]
        cql: String,
        /// Maximum number of results to return (default: 25)
        #[arg(long, default_value = "25")]
        limit: u32,
        /// Offset into the result set, for pagination (default: 0)
        #[arg(long, default_value = "0")]
        start: u32,
    },
    /// Delete a page — requires --confirm
    ///
    /// By default moves the page to the trash, where it can be restored —
    /// not a permanent delete. Pass --purge to permanently remove it instead,
    /// but this only works on a page that is already trashed: to permanently
    /// delete a page in one workflow, call this command twice — once without
    /// --purge (moves it to trash), then again with --purge (purges it).
    /// Always prints its full result regardless of --select — a small,
    /// synthesized confirmation object.
    #[command(after_help = "Examples:\n  confluence page delete 123456 --confirm\n  confluence page delete 123456 --confirm --purge\n\nThis command has no output body from the Confluence API (204 No Content) —\nthe printed JSON is synthesized by this CLI to confirm what happened.")]
    Delete {
        /// Page ID to delete
        id: String,
        /// Acknowledge that this action moves the page to trash (or, with --purge, permanently deletes it)
        #[arg(long)]
        confirm: bool,
        /// Permanently delete instead of trashing — only works on a page that is already trashed
        #[arg(long)]
        purge: bool,
    },
}

#[derive(Debug, Subcommand)]
pub enum SpaceCommand {
    /// List Confluence spaces and print them as JSON
    #[command(after_help = "Examples:\n  confluence space list\n  confluence space list --limit 10\n  confluence space list --cursor <cursor-from-previous-response>\n\nPagination: the response's _links.next field (if present) contains a cursor\nquery parameter — pass its value to --cursor to fetch the next page.")]
    List {
        /// Maximum number of spaces to return (default: 25)
        #[arg(long, default_value = "25")]
        limit: u32,
        /// Cursor token for the next page, from the _links.next field of a previous response
        #[arg(long)]
        cursor: Option<String>,
    },
}

#[derive(Debug, Subcommand)]
pub enum TemplateCommand {
    /// Create a new content template
    ///
    /// Exactly one of --body or --body-file supplies the template content
    /// (same storage-format XHTML as `page create`'s --body/--body-file — see
    /// that command's help). Omit --space-key to create a global template
    /// (requires Confluence Administrator global permission); pass it to
    /// create a space template instead (requires Admin permission on that
    /// space). The created template's ID (`templateId` in the response) can
    /// then be passed to `page create --template-id` to build pages from it.
    #[command(after_help = "Examples:\n  confluence template create --name \"Runbook\" --space-key ENG --body \"<p>Steps</p>\"\n  confluence template create --name \"Postmortem\" --body-file ./postmortem.html --description \"Standard postmortem layout\"")]
    Create {
        /// Space key to create a space-scoped template in; omit for a global template
        #[arg(long)]
        space_key: Option<String>,
        /// Template name
        #[arg(long)]
        name: String,
        /// Optional template description
        #[arg(long)]
        description: Option<String>,
        /// Template body as raw Confluence storage-format XHTML
        #[arg(long, conflicts_with = "body_file")]
        body: Option<String>,
        /// Path to a local file whose content becomes the template body
        #[arg(long, conflicts_with = "body")]
        body_file: Option<String>,
    },
    /// List content templates and print them as JSON
    #[command(after_help = "Examples:\n  confluence template list\n  confluence template list --space-key ENG\n  confluence template list --limit 10 --start 10\n\nOffset pagination: pass --start <previous start + limit> to fetch the next page.")]
    List {
        /// Only list templates in this space; omit to list global templates
        #[arg(long)]
        space_key: Option<String>,
        /// Maximum number of templates to return (default: 25)
        #[arg(long, default_value = "25")]
        limit: u32,
        /// Offset into the result set, for pagination (default: 0)
        #[arg(long, default_value = "0")]
        start: u32,
    },
    /// Update an existing template's name, description, and/or body
    ///
    /// Confluence's template API replaces the whole template on every
    /// update — there is no partial-patch endpoint. This command fetches the
    /// template's current name/description/body first, then submits a full
    /// update with your --name/--description/--body(-file) overriding just
    /// those fields (the others keep their current value). At least one of
    /// --name, --description, --body, or --body-file is required.
    #[command(after_help = "Examples:\n  confluence template update 4321 --name \"Runbook (v2)\"\n  confluence template update 4321 --body-file ./runbook-v2.html")]
    Update {
        /// Template ID
        id: String,
        /// New template name
        #[arg(long)]
        name: Option<String>,
        /// New template description
        #[arg(long)]
        description: Option<String>,
        /// New template body as raw Confluence storage-format XHTML
        #[arg(long, conflicts_with = "body_file")]
        body: Option<String>,
        /// Path to a local file whose content becomes the new template body
        #[arg(long, conflicts_with = "body")]
        body_file: Option<String>,
    },
    /// Permanently delete a template — requires --confirm
    ///
    /// Always prints its full result regardless of --select — a small,
    /// synthesized confirmation object.
    #[command(after_help = "Example: confluence template delete 4321 --confirm\n\nThis action is irreversible. --confirm must be passed explicitly so the caller acknowledges the deletion.")]
    Delete {
        /// Template ID to delete
        id: String,
        /// Acknowledge that this action is permanent and irreversible
        #[arg(long)]
        confirm: bool,
    },
}

#[cfg(test)]
#[path = "tests/cli_tests.rs"]
mod tests;
