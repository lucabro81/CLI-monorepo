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
    /// Required on most commands: if both this and --select-all are omitted, the
    /// command fails with an error reporting the byte size of the full response and
    /// its top-level field names, so you can retry with an informed --select. A few
    /// commands whose output is always small and fixed-shape (doctor, auth whoami,
    /// repo get/create/delete, pr get/create/approve/unapprove/decline/merge/comment)
    /// are exempt and print in full regardless — see that command's own --help.
    /// Example: --select `uuid,display_name`
    #[arg(long, global = true, value_name = "PATHS", conflicts_with = "select_all")]
    pub select: Option<String>,

    /// Explicitly print the full, unfiltered JSON response instead of specifying --select.
    /// Use when you already know the response is small; otherwise prefer --select.
    #[arg(long, global = true, conflicts_with = "select")]
    pub select_all: bool,

    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Guided onboarding: create app.json and run the first login
    #[command(after_help = "Example:\n  bitbucket init\n  bitbucket init --client-id ABC123 --client-secret xyz")]
    Init {
        /// Bitbucket OAuth consumer Key (skips interactive prompt if provided)
        #[arg(long)]
        client_id: Option<String>,
        /// Bitbucket OAuth consumer Secret (skips interactive prompt if provided)
        #[arg(long)]
        client_secret: Option<String>,
    },
    /// Check that the CLI is correctly configured and can reach the Bitbucket API
    ///
    /// Always prints its full result regardless of --select — the report is
    /// generated internally and is always small and fixed-shape.
    #[command(after_help = "Example:\n  bitbucket doctor")]
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
    ///
    /// Always prints its full result regardless of --select — an identity check,
    /// small and fixed-shape.
    #[command(after_help = "Examples:\n  bitbucket auth whoami\n  bitbucket auth whoami --select uuid,display_name")]
    Whoami,
}

#[derive(Debug, Subcommand)]
pub enum RepoCommand {
    /// Print repository details as JSON
    ///
    /// Always prints its full result regardless of --select — a single repository
    /// object, fixed-shape.
    #[command(after_help = "Examples:\n  bitbucket repo get lucabrognaracode/my-repo\n  bitbucket repo get lucabrognaracode/my-repo --select description,language")]
    Get {
        /// Full repository identifier in the form `workspace/repo_slug`
        repository: String,
    },
    /// List repositories in a workspace, as JSON
    #[command(after_help = "Examples:\n  bitbucket repo list lucabrognaracode\n  bitbucket repo list lucabrognaracode --page 2\n  bitbucket repo list lucabrognaracode --select values.full_name")]
    List {
        /// Workspace slug, e.g. `lucabrognaracode`
        workspace: String,
        /// Page number to fetch (Bitbucket pagination starts at 1)
        #[arg(long)]
        page: Option<u32>,
    },
    /// Create a new repository, as JSON
    ///
    /// Always prints its full result regardless of --select — a single repository
    /// object, fixed-shape.
    #[command(after_help = "Examples:\n  bitbucket repo create lucabrognaracode/my-new-repo\n  bitbucket repo create lucabrognaracode/my-new-repo --description \"My new repo\" --private\n  bitbucket repo create lucabrognaracode/my-new-repo --project PROJ")]
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
    #[command(after_help = "Example:\n  bitbucket repo delete lucabrognaracode/my-repo --confirm")]
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
    #[command(after_help = "Examples:\n  bitbucket pr get lucabrognaracode/my-repo 42\n  bitbucket pr get lucabrognaracode/my-repo 42 --select title,state,source.branch.name")]
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
    #[command(after_help = "Examples:\n  bitbucket pr create lucabrognaracode/my-repo --title \"My PR\" --source feature-branch\n  bitbucket pr create lucabrognaracode/my-repo --title \"My PR\" --source feature-branch --destination main --description \"does things\"\n  bitbucket pr create lucabrognaracode/my-repo --title \"My PR\" --source feature-branch --close-source-branch")]
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
    },
    /// Approve a pull request, as JSON
    ///
    /// Always prints its full result regardless of --select — a small approval
    /// object.
    #[command(after_help = "Example:\n  bitbucket pr approve lucabrognaracode/my-repo 42")]
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
    #[command(after_help = "Example:\n  bitbucket pr unapprove lucabrognaracode/my-repo 42")]
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
    #[command(after_help = "Example:\n  bitbucket pr decline lucabrognaracode/my-repo 42 --confirm")]
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
    #[command(after_help = "Examples:\n  bitbucket pr merge lucabrognaracode/my-repo 42 --confirm\n  bitbucket pr merge lucabrognaracode/my-repo 42 --merge-strategy squash --close-source-branch --confirm")]
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
    /// Add a comment to a pull request, as JSON
    ///
    /// Always prints its full result regardless of --select — a single comment
    /// object, fixed-shape.
    #[command(after_help = "Examples:\n  bitbucket pr comment lucabrognaracode/my-repo 42 --content \"Looks good to me\"\n  bitbucket pr comment lucabrognaracode/my-repo 42 --content \"Fix this\" --path src/main.rs --line 10")]
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
    },
    /// List pull requests in a repository, as JSON
    #[command(after_help = "Examples:\n  bitbucket pr list lucabrognaracode/my-repo\n  bitbucket pr list lucabrognaracode/my-repo --state MERGED\n  bitbucket pr list lucabrognaracode/my-repo --page 2\n  bitbucket pr list lucabrognaracode/my-repo --select values.title,values.state")]
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
    #[command(after_help = "Examples:\n  bitbucket pr diff lucabrognaracode/my-repo 42\n  bitbucket pr diff lucabrognaracode/my-repo 42 --context 5\n  bitbucket pr diff lucabrognaracode/my-repo 42 --path src/main.rs")]
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
    #[command(after_help = "Examples:\n  bitbucket branch list lucabrognaracode/my-repo\n  bitbucket branch list lucabrognaracode/my-repo --page 2\n  bitbucket branch list lucabrognaracode/my-repo --select values.name")]
    List {
        /// Full repository identifier in the form `workspace/repo_slug`
        repository: String,
        /// Page number to fetch (Bitbucket pagination starts at 1)
        #[arg(long)]
        page: Option<u32>,
    },
}

#[cfg(test)]
#[path = "tests/cli_tests.rs"]
mod tests;
