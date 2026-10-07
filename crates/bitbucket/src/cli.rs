//! CLI surface definition — all clap structs and enums.
//!
//! No logic lives here — this file is purely argument parsing and help text.
//! Every flag uses `#[arg(long)]` only; no short aliases.

use clap::{Parser, Subcommand};
use oauth_user_login::UserId;

/// Bitbucket CLI for LLM agents — query Bitbucket Cloud from the command line.
#[derive(Debug, Parser)]
#[command(name = "bitbucket", version, about)]
pub struct Cli {
    /// Comma-separated dot-notation paths to project from the JSON output (client-side).
    /// Required on most commands: if both this and --select-all are omitted, the
    /// command fails with an error reporting the byte size of the full response and
    /// its top-level field names, so you can retry with an informed --select. A few
    /// commands whose output is always small and fixed-shape (doctor, auth whoami, auth logout,
    /// repo get/create/delete, pr get/create/approve/unapprove/decline/merge/comment/update-comment,
    /// branch create, branch suggest-name) are exempt and print in full
    /// regardless — see that command's own --help.
    /// Example: --select `uuid,display_name`
    #[arg(long, global = true, value_name = "PATHS", conflicts_with = "select_all")]
    pub select: Option<String>,

    /// Explicitly print the full, unfiltered JSON response instead of specifying --select.
    /// Use when you already know the response is small; otherwise prefer --select. Still
    /// refused if the response exceeds a fixed byte cap (currently 30000 bytes) — the error
    /// reports the actual size and top-level fields so you can retry with --select instead.
    #[arg(long, global = true, conflicts_with = "select")]
    pub select_all: bool,

    /// Act as the person with this id instead of the OAuth app: the human who
    /// logged in with `bitbucket auth login --user <USER_ID>`. The id is your own
    /// name for that person, a lowercase slug (a-z, 0-9, '.', '_', '-', ':', e.g.
    /// jane.doe or chat:u123). Without it every command acts as the OAuth app (`bitbucket auth
    /// login`). The OAuth app and every person are stored side by side and renewed
    /// automatically, so switching between them needs no new login. On `init`,
    /// `auth login`, `auth whoami`, `auth logout` and `doctor` it selects which
    /// identity to set up, log in, check or remove.
    #[arg(long, global = true, value_name = "USER_ID", value_parser = UserId::parse)]
    pub user: Option<UserId>,

    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Onboarding: save an identity's OAuth consumer to app.json, log in, verify with doctor
    ///
    /// Without --user: writes app.json's "service" section and runs the
    /// `client_credentials` login (acting as the OAuth app, no browser). With
    /// `--user <USER_ID>`: writes the "user" section (the consumer every person logs
    /// in through) and runs the browser login for that person. The same consumer
    /// may be used for both. The other section of app.json is left untouched. Then
    /// prints a doctor JSON report for that identity. Pass --client-id and
    /// --client-secret to skip interactive prompts. To log in more people once the
    /// "user" section is set up, use `bitbucket auth login --user <USER_ID>`.
    #[command(after_help = "Examples:\n  bitbucket init --client-id <KEY> --client-secret <SECRET>          # the OAuth app\n  bitbucket init --user jane.doe --client-id <KEY> --client-secret <SECRET>   # jane.doe, browser consent\n  bitbucket init                                                     # interactive prompts")]
    Init {
        /// Bitbucket OAuth consumer Key of the identity being set up (skips interactive prompt if provided)
        #[arg(long)]
        client_id: Option<String>,
        /// Bitbucket OAuth consumer Secret of the identity being set up (skips interactive prompt if provided)
        #[arg(long)]
        client_secret: Option<String>,
    },
    /// Check that the CLI is correctly configured and can reach the Bitbucket API
    ///
    /// Checks the OAuth app, or the person with `--user <USER_ID>`. Also reports
    /// `pending_login` (that person's two-step `auth login --user <USER_ID> --remote`
    /// waiting for its code) and `identities` (whether the OAuth app is logged in,
    /// the ids of the people logged in, and credentials files of earlier layouts,
    /// credentials.json and credentials-user.json, no longer read): informational,
    /// never counted in the exit code.
    /// Always prints its full result regardless of --select — the report is
    /// generated internally and is always small and fixed-shape.
    #[command(after_help = "Examples:\n  bitbucket doctor\n  bitbucket doctor --user jane.doe")]
    Doctor,
    /// Manage authentication with Bitbucket
    Auth {
        #[command(subcommand)]
        command: AuthCommand,
    },
    /// Inspect repositories
    Repo {
        #[command(subcommand)]
        command: RepoCommand,
    },
    /// Inspect pull requests
    Pr {
        #[command(subcommand)]
        command: PrCommand,
    },
    /// Inspect branches
    Branch {
        #[command(subcommand)]
        command: BranchCommand,
    },
    /// Inspect workspaces
    Workspace {
        #[command(subcommand)]
        command: WorkspaceCommand,
    },
}

#[derive(Debug, Subcommand)]
pub enum AuthCommand {
    /// Run the OAuth 2.0 login flow and store credentials locally
    ///
    /// By default runs the `client_credentials` flow: exchanges the OAuth consumer's
    /// `client_id`/`client_secret` (app.json's "service" section) for an access token. No browser,
    /// no user interaction. Every action is attributed to the OAuth app (bot
    /// identity) — the expected mode for agent-driven usage.
    ///
    /// Pass `--user <USER_ID>` for the interactive `authorization_code` flow, with
    /// app.json's "user" section (the consumer every person logs in through): opens the browser
    /// on Bitbucket's consent page, receives the callback on localhost:8080, and
    /// stores a `refresh_token` for automatic renewal. Every action is attributed
    /// to the human Bitbucket account that approved the consent page. Requires the
    /// OAuth consumer's callback URL to be `http://localhost:8080/callback`.
    ///
    /// With `--user <USER_ID> --remote`: a two-step login for a person who is not at this
    /// machine. Step 1 (--remote) opens no browser and listens on no port: it
    /// prints JSON `{authorize_url, state, expires_at}` for the caller to hand to
    /// the person. Bitbucket has no redirect URI parameter: it always redirects
    /// to the OAuth consumer's callback URL, so remote logins need a consumer
    /// whose callback URL is the caller's own endpoint, in the "user" section (a
    /// browser login on this machine instead needs the localhost callback). Step 2 (--code --state) exchanges
    /// the code, saves the credentials, then prints what `auth whoami --user <USER_ID>`
    /// prints. The pending login expires after 10 minutes, its state is single-use,
    /// and it lives in that person's folder of this config folder (`XDG_CONFIG_HOME`),
    /// so several people can be mid-login at once.
    ///
    /// Each login replaces only its own identity's credentials
    /// (credentials-service.json / `users/<USER_ID>/credentials.json`): afterwards
    /// every command acts as the OAuth app, or as that person when it is given
    /// `--user <USER_ID>`. Run this once per identity; tokens are renewed
    /// automatically after that (one renewal at a time per identity, so parallel
    /// calls are safe).
    #[command(after_help = "Examples:\n  bitbucket auth login           # act as the OAuth app (bot)\n  bitbucket auth login --user jane.doe    # act as jane.doe, via browser consent\n  bitbucket auth login --user jane.doe --remote                           # step 1: prints the consent URL\n  bitbucket auth login --user jane.doe --code <CODE> --state <STATE>      # step 2\n\nRequires app.json at ~/.config/bitbucket-cli/app.json with the identity's section.\nRun `bitbucket init` (or `bitbucket init --user <USER_ID>`) first if it is missing.")]
    Login {
        /// Step 1 of a two-step login for someone not at this machine: print the consent URL instead of opening a browser
        // "needs --user <USER_ID>" is checked in LoginMode::from_flags: clap's
        // `requires` cannot see a global --user written before the subcommand.
        #[arg(long, conflicts_with_all = ["code", "state"])]
        remote: bool,
        /// Step 2: the `code` query parameter Bitbucket appended to the consumer's callback URL
        #[arg(long, requires = "state")]
        code: Option<String>,
        /// Step 2: the `state` query parameter Bitbucket appended to the consumer's callback URL
        #[arg(long, requires = "code")]
        state: Option<String>,
    },
    /// Print the account the CLI acts as, as JSON: the OAuth app, or the person with `--user <USER_ID>`
    ///
    /// Always prints its full result regardless of --select — an identity check,
    /// small and fixed-shape.
    #[command(after_help = "Examples:\n  bitbucket auth whoami\n  bitbucket auth whoami --user jane.doe\n  bitbucket auth whoami --select uuid,display_name")]
    Whoami,
    /// Remove the stored login of the OAuth app, or of the person with `--user <USER_ID>`
    ///
    /// Deletes that identity's credentials (for a person, their whole folder,
    /// including a pending remote login) from this machine, so commands can no
    /// longer act as it until it logs in again. Other identities are untouched.
    /// app.json is kept. Local only: the tokens are not revoked at Bitbucket (a
    /// person can revoke the consumer's access from their Bitbucket account).
    /// Prints JSON `{"logged_out": "service"}` or `{"logged_out": "user:<USER_ID>"}`;
    /// fails if that identity had no stored login.
    #[command(after_help = "Examples:\n  bitbucket auth logout --user jane.doe\n  bitbucket auth logout")]
    Logout,
}

#[derive(Debug, Subcommand)]
pub enum RepoCommand {
    /// Print repository details as JSON
    ///
    /// Always prints its full result regardless of --select — a single repository
    /// object, fixed-shape.
    #[command(after_help = "Examples:\n  bitbucket repo get <workspace>/my-repo\n  bitbucket repo get <workspace>/my-repo --select description,language")]
    Get {
        /// Full repository identifier in the form `workspace/repo_slug`
        repository: String,
    },
    /// List repositories in a workspace, as JSON
    #[command(after_help = "Examples:\n  bitbucket repo list <workspace> --select values.full_name\n  bitbucket repo list <workspace> --page 2 --select values.full_name\n  bitbucket repo list <workspace> --select-all")]
    List {
        /// Workspace slug
        workspace: String,
        /// Page number to fetch (Bitbucket pagination starts at 1)
        #[arg(long)]
        page: Option<u32>,
    },
    /// Create a new repository, as JSON
    ///
    /// Always prints its full result regardless of --select — a single repository
    /// object, fixed-shape.
    #[command(after_help = "Examples:\n  bitbucket repo create <workspace>/my-new-repo\n  bitbucket repo create <workspace>/my-new-repo --description \"My new repo\" --private\n  bitbucket repo create <workspace>/my-new-repo --project PROJ")]
    Create {
        /// Full repository identifier in the form `workspace/repo_slug`
        repository: String,
        /// Optional repository description
        #[arg(long)]
        description: Option<String>,
        /// Create as a private repository (default: workspace default)
        #[arg(long)]
        private: bool,
        /// Project key to assign the repository to, e.g. PROJ
        #[arg(long)]
        project: Option<String>,
    },
    /// Delete a repository, as JSON
    ///
    /// This permanently deletes the repository and cannot be undone. Always prints
    /// its full result regardless of --select — a small, synthesized confirmation
    /// object.
    #[command(after_help = "Example:\n  bitbucket repo delete <workspace>/my-repo --confirm")]
    Delete {
        /// Full repository identifier in the form `workspace/repo_slug`
        repository: String,
        /// Confirm the deletion (required — this is permanent)
        #[arg(long)]
        confirm: bool,
    },
}

#[derive(Debug, Subcommand)]
pub enum PrCommand {
    /// Print pull request details as JSON
    ///
    /// Always prints its full result regardless of --select — a single pull
    /// request object, fixed-shape.
    #[command(after_help = "Examples:\n  bitbucket pr get <workspace>/my-repo 42\n  bitbucket pr get <workspace>/my-repo 42 --select title,state,source.branch.name")]
    Get {
        /// Full repository identifier in the form `workspace/repo_slug`
        repository: String,
        /// Pull request ID
        id: u64,
    },
    /// Create a new pull request, as JSON
    ///
    /// Always prints its full result regardless of --select — a single pull
    /// request object, fixed-shape.
    #[command(after_help = "Examples:\n  bitbucket pr create <workspace>/my-repo --title \"My PR\" --source feature-branch\n  bitbucket pr create <workspace>/my-repo --title \"My PR\" --source feature-branch --destination main --description \"does things\"\n  bitbucket pr create <workspace>/my-repo --title \"My PR\" --source feature-branch --close-source-branch\n  bitbucket pr create <workspace>/my-repo --title \"My PR\" --source feature-branch --reviewers \"{xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx}\"\n  bitbucket pr create <workspace>/my-repo --title \"WIP: My PR\" --source feature-branch --draft")]
    Create {
        /// Full repository identifier in the form `workspace/repo_slug`
        repository: String,
        /// Pull request title
        #[arg(long)]
        title: String,
        /// Source branch name (the branch containing the changes)
        #[arg(long)]
        source: String,
        /// Destination branch name. If omitted, Bitbucket uses the repository's main branch.
        #[arg(long)]
        destination: Option<String>,
        /// Pull request description
        #[arg(long)]
        description: Option<String>,
        /// Close the source branch after the pull request is merged
        #[arg(long)]
        close_source_branch: bool,
        /// Comma-separated list of reviewer UUIDs, each formatted with curly braces
        /// (e.g. "{xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx}"). Find UUIDs with
        /// `bitbucket workspace members <workspace>`.
        #[arg(long)]
        reviewers: Option<String>,
        /// Create the pull request as a draft (not yet ready for review).
        /// Publish it later with `bitbucket pr update <repository> <id> --ready-for-review`.
        #[arg(long)]
        draft: bool,
    },
    /// Update an open pull request's title, description, destination branch,
    /// reviewers, or draft status, as JSON
    ///
    /// The pull request must be open. Only the fields you pass are changed, with
    /// one exception: --reviewers replaces the entire reviewer list rather than
    /// adding to it — to add a reviewer to an existing list, pass all current
    /// reviewer UUIDs plus the new one. Always prints its full result regardless
    /// of --select — a single pull request object, fixed shape.
    #[command(after_help = "Examples:\n  bitbucket pr update <workspace>/my-repo 42 --title \"New title\"\n  bitbucket pr update <workspace>/my-repo 42 --description \"Updated description\"\n  bitbucket pr update <workspace>/my-repo 42 --destination develop\n  bitbucket pr update <workspace>/my-repo 42 --reviewers \"{xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx}\"\n  bitbucket pr update <workspace>/my-repo 42 --draft\n  bitbucket pr update <workspace>/my-repo 42 --ready-for-review")]
    Update {
        /// Full repository identifier in the form `workspace/repo_slug`
        repository: String,
        /// Pull request ID
        id: u64,
        /// New pull request title
        #[arg(long)]
        title: Option<String>,
        /// New pull request description
        #[arg(long)]
        description: Option<String>,
        /// New destination branch name
        #[arg(long)]
        destination: Option<String>,
        /// Comma-separated list of reviewer UUIDs, each formatted with curly braces
        /// (e.g. "{xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx}"). Find UUIDs with
        /// `bitbucket workspace members <workspace>`. Replaces the entire reviewer
        /// list rather than adding to it.
        #[arg(long)]
        reviewers: Option<String>,
        /// Convert the pull request to a draft. Cannot be combined with --ready-for-review.
        #[arg(long, conflicts_with = "ready_for_review")]
        draft: bool,
        /// Mark a draft pull request as ready for review (publish it). Cannot be
        /// combined with --draft.
        #[arg(long)]
        ready_for_review: bool,
    },
    /// Approve a pull request, as JSON
    ///
    /// Always prints its full result regardless of --select — a small approval
    /// object.
    #[command(after_help = "Example:\n  bitbucket pr approve <workspace>/my-repo 42")]
    Approve {
        /// Full repository identifier in the form `workspace/repo_slug`
        repository: String,
        /// Pull request ID
        id: u64,
    },
    /// Remove your approval from a pull request, as JSON
    ///
    /// Always prints its full result regardless of --select — a small, synthesized
    /// confirmation object.
    #[command(after_help = "Example:\n  bitbucket pr unapprove <workspace>/my-repo 42")]
    Unapprove {
        /// Full repository identifier in the form `workspace/repo_slug`
        repository: String,
        /// Pull request ID
        id: u64,
    },
    /// Decline a pull request, as JSON
    ///
    /// This changes the pull request's state and cannot be undone by this CLI.
    /// Always prints its full result regardless of --select — a single pull
    /// request object, fixed-shape.
    #[command(after_help = "Example:\n  bitbucket pr decline <workspace>/my-repo 42 --confirm")]
    Decline {
        /// Full repository identifier in the form `workspace/repo_slug`
        repository: String,
        /// Pull request ID
        id: u64,
        /// Confirm the decline (required — this changes the pull request's state)
        #[arg(long)]
        confirm: bool,
    },
    /// Merge a pull request, as JSON
    ///
    /// This is permanent and cannot be undone. Always prints its full result
    /// regardless of --select — a single pull request object, fixed-shape.
    #[command(after_help = "Examples:\n  bitbucket pr merge <workspace>/my-repo 42 --confirm\n  bitbucket pr merge <workspace>/my-repo 42 --merge-strategy squash --close-source-branch --confirm")]
    Merge {
        /// Full repository identifier in the form `workspace/repo_slug`
        repository: String,
        /// Pull request ID
        id: u64,
        /// Custom merge commit message. If omitted, Bitbucket generates a default message.
        #[arg(long)]
        message: Option<String>,
        /// Merge strategy: `merge_commit`, `squash`, or `fast_forward`. If omitted, Bitbucket uses the repository's default.
        #[arg(long)]
        merge_strategy: Option<String>,
        /// Close the source branch after merging
        #[arg(long)]
        close_source_branch: bool,
        /// Confirm the merge (required — this is permanent)
        #[arg(long)]
        confirm: bool,
    },
    /// Add a comment (general, inline, or a reply) to a pull request, as JSON
    ///
    /// Always prints its full result regardless of --select — a single comment
    /// object, fixed-shape.
    #[command(after_help = "Examples:\n  bitbucket pr comment <workspace>/my-repo 42 --content \"Looks good to me\"\n  bitbucket pr comment <workspace>/my-repo 42 --content \"Fix this\" --path src/main.rs --line 10\n  bitbucket pr comment <workspace>/my-repo 42 --content \"Done, fixed\" --parent 123456")]
    Comment {
        /// Full repository identifier in the form `workspace/repo_slug`
        repository: String,
        /// Pull request ID
        id: u64,
        /// Comment text (Markdown)
        #[arg(long)]
        content: String,
        /// File path to attach an inline comment to. Requires --line.
        #[arg(long)]
        path: Option<String>,
        /// Line number in the new version of the file to attach an inline comment to. Requires --path.
        #[arg(long)]
        line: Option<u64>,
        /// ID of the comment to reply to (the `id` field from `bitbucket pr list-comments`).
        /// The reply goes into the parent's thread; a reply to an inline comment stays on
        /// the parent's file and line, so this cannot be combined with --path/--line.
        #[arg(long, conflicts_with_all = ["path", "line"])]
        parent: Option<u64>,
    },
    /// List all comments on a pull request, as JSON
    ///
    /// Returns general comments, inline comments, and replies, oldest first.
    /// Deleted comments are included too, marked with `deleted: true`. Replies
    /// carry `parent.id`; inline comments carry `inline.path` and `inline.to`.
    #[command(after_help = "Examples:\n  bitbucket pr list-comments <workspace>/my-repo 42 --select values.id,values.content.raw,values.user.display_name,values.deleted,values.inline.path\n  bitbucket pr list-comments <workspace>/my-repo 42 --page 2 --select values.id,values.content.raw")]
    ListComments {
        /// Full repository identifier in the form `workspace/repo_slug`
        repository: String,
        /// Pull request ID
        id: u64,
        /// Page number to fetch (Bitbucket pagination starts at 1)
        #[arg(long)]
        page: Option<u32>,
    },
    /// Replace the text of an existing pull request comment, as JSON
    ///
    /// Only the comment text changes; an inline comment keeps its file and line.
    /// Bitbucket normally lets only the comment's author edit it. Find comment
    /// IDs with `bitbucket pr list-comments`. Always prints its full result
    /// regardless of --select — a single comment object, fixed shape.
    #[command(after_help = "Example:\n  bitbucket pr update-comment <workspace>/my-repo 42 123456 --content \"Updated: looks good to me\"")]
    UpdateComment {
        /// Full repository identifier in the form `workspace/repo_slug`
        repository: String,
        /// Pull request ID
        id: u64,
        /// Comment ID (the `id` field from `bitbucket pr list-comments`)
        comment_id: u64,
        /// New comment text (Markdown). Replaces the existing text entirely.
        #[arg(long)]
        content: String,
    },
    /// List pull requests in a repository, as JSON
    #[command(after_help = "Examples:\n  bitbucket pr list <workspace>/my-repo --select values.id,values.title,values.state\n  bitbucket pr list <workspace>/my-repo --state MERGED --select values.id,values.title\n  bitbucket pr list <workspace>/my-repo --page 2 --select values.id,values.title\n  bitbucket pr list <workspace>/my-repo --select-all")]
    List {
        /// Full repository identifier in the form `workspace/repo_slug`
        repository: String,
        /// Filter by pull request state: OPEN, MERGED, DECLINED, or SUPERSEDED.
        /// If omitted, Bitbucket returns pull requests in all states.
        #[arg(long)]
        state: Option<String>,
        /// Page number to fetch (Bitbucket pagination starts at 1)
        #[arg(long)]
        page: Option<u32>,
    },
    /// Print the raw unified diff for a pull request
    ///
    /// Prints the diff as plain text (unified diff format), not JSON — `--select`
    /// has no effect on this command.
    #[command(after_help = "Examples:\n  bitbucket pr diff <workspace>/my-repo 42\n  bitbucket pr diff <workspace>/my-repo 42 --context 5\n  bitbucket pr diff <workspace>/my-repo 42 --path src/main.rs")]
    Diff {
        /// Full repository identifier in the form `workspace/repo_slug`
        repository: String,
        /// Pull request ID
        id: u64,
        /// Number of unchanged context lines to show around each change.
        /// If omitted, Bitbucket uses its default.
        #[arg(long)]
        context: Option<u32>,
        /// Restrict the diff to a single file path
        #[arg(long)]
        path: Option<String>,
    },
}

#[derive(Debug, Subcommand)]
pub enum BranchCommand {
    /// List branches in a repository, as JSON
    #[command(after_help = "Examples:\n  bitbucket branch list <workspace>/my-repo --select values.name\n  bitbucket branch list <workspace>/my-repo --page 2 --select values.name\n  bitbucket branch list <workspace>/my-repo --select-all")]
    List {
        /// Full repository identifier in the form `workspace/repo_slug`
        repository: String,
        /// Page number to fetch (Bitbucket pagination starts at 1)
        #[arg(long)]
        page: Option<u32>,
    },
    /// Create a new branch in a repository, as JSON
    ///
    /// Always prints its full result regardless of --select — a single
    /// branch object, fixed-shape.
    #[command(after_help = "Examples:\n  bitbucket branch create <workspace>/my-repo feature/my-branch --target main\n  bitbucket branch create <workspace>/my-repo feature/SBF-19-my-fix --target develop")]
    Create {
        /// Full repository identifier in the form `workspace/repo_slug`
        repository: String,
        /// Name for the new branch
        name: String,
        /// Branch name or commit hash to create the new branch from
        #[arg(long)]
        target: String,
    },
    /// Compute a suggested Bitbucket branch name from a Jira issue's
    /// key/type/summary, as JSON
    ///
    /// Prefix resolution: --prefix wins if given (no network call). Else, if
    /// --repository is given, looks up that repo's actual branching model
    /// (GET .../branching-model) for the real configured prefix — requires
    /// authentication. Else, falls back to a local heuristic (Bug -> bugfix,
    /// anything else -> feature), no network call, no auth. Always prints its
    /// full result regardless of --select — a small fixed-shape object.
    #[command(after_help = "Examples:\n  bitbucket branch suggest-name --issue-key SBF-19 --issue-type Task --issue-summary \"Costruire griglia Smartlocker v2\"\n  bitbucket branch suggest-name --issue-key SBF-19 --issue-type Task --issue-summary \"...\" --repository <workspace>/my-repo\n  bitbucket branch suggest-name --issue-key SBF-19 --issue-type Task --issue-summary \"...\" --prefix hotfix")]
    SuggestName {
        /// Jira issue key, e.g. SBF-19
        #[arg(long)]
        issue_key: String,
        /// Jira issue type, e.g. Task, Bug, Story
        #[arg(long)]
        issue_type: String,
        /// Jira issue summary
        #[arg(long)]
        issue_summary: String,
        /// Repository to look up the real branching model from, in the form
        /// `workspace/repo_slug`. Ignored if --prefix is also given.
        #[arg(long)]
        repository: Option<String>,
        /// Explicit prefix override — skips both inference and any lookup
        #[arg(long)]
        prefix: Option<String>,
    },
}

#[derive(Debug, Subcommand)]
pub enum WorkspaceCommand {
    /// List members of a workspace, as JSON
    #[command(after_help = "Examples:\n  bitbucket workspace members <workspace> --select values.user.uuid,values.user.display_name\n  bitbucket workspace members <workspace> --page 2 --select values.user.uuid,values.user.display_name\n  bitbucket workspace members <workspace> --select-all")]
    Members {
        /// Workspace slug
        workspace: String,
        /// Page number to fetch (Bitbucket pagination starts at 1)
        #[arg(long)]
        page: Option<u32>,
    },
}

#[cfg(test)]
#[path = "tests/cli_tests.rs"]
mod tests;
