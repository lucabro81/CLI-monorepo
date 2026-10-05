//! CLI surface definition — all clap structs and enums. No logic.
//!
//! Every flag uses `#[arg(long)]` only; no short aliases.

use clap::{Parser, Subcommand, ValueEnum};

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
    /// Work with users (human and service users) of the instance
    User {
        #[command(subcommand)]
        command: UserCommand,
    },
    /// Work with organizations of the instance
    Organization {
        #[command(subcommand)]
        command: OrganizationCommand,
    },
    /// Work with projects (containers for applications and roles)
    Project {
        #[command(subcommand)]
        command: ProjectCommand,
    },
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
    /// Default: logs in as the service user configured in app.json (private key
    /// JWT) — no browser, no human interaction; this is the mode for agents.
    /// With --user: logs in as a human through the browser (authorization code +
    /// PKCE on the Native app configured with `init --client-id`; listens on
    /// localhost:8080 for the redirect). Either way the token is renewed
    /// automatically, so this is normally needed once; the last login decides
    /// which identity the CLI acts as. What it may then do is decided by the
    /// ZITADEL administrator roles of that identity (e.g. `IAM_OWNER`, `ORG_OWNER`).
    ///
    /// With --user --remote: a two-step login for a person who is not at this
    /// machine. Step 1 (--remote --redirect-uri) opens no browser and listens on no
    /// port: it prints JSON `{authorize_url, state, expires_at}` for the caller to hand
    /// to the person. The provider then redirects the person to --redirect-uri (it
    /// must be registered on the Native app) with `code` and `state`. Step 2
    /// (--code --state) exchanges the code and saves the credentials, then prints
    /// what `auth whoami` prints. The pending login expires after 10 minutes, its
    /// state is single-use, and it lives in this config folder (`XDG_CONFIG_HOME`).
    #[command(after_help = "Examples:\n  zitadel auth login          # service user (agents)\n  zitadel auth login --user   # yourself, via the browser\n  zitadel auth login --user --remote --redirect-uri https://app.example.com/oauth/callback   # step 1\n  zitadel auth login --user --code <CODE> --state <STATE>                                   # step 2")]
    Login {
        /// Log in as a human via the browser instead of as the service user
        #[arg(long)]
        user: bool,
        /// Step 1 of a two-step login for someone not at this machine: print the authorize URL instead of opening a browser
        #[arg(long, requires_all = ["user", "redirect_uri"], conflicts_with_all = ["code", "state"])]
        remote: bool,
        /// With --remote: where the provider sends the person back; must be registered on the Native app
        #[arg(long, requires = "remote")]
        redirect_uri: Option<String>,
        /// Step 2: the `code` query parameter the provider appended to the redirect URI
        #[arg(long, requires_all = ["user", "state"])]
        code: Option<String>,
        /// Step 2: the `state` query parameter the provider appended to the redirect URI
        #[arg(long, requires = "code")]
        state: Option<String>,
    },
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

#[derive(Debug, Subcommand)]
pub enum UserCommand {
    /// Search users across the organizations the identity may read
    ///
    /// Calls ZITADEL's v2 `ListUsers` (POST /v2/users). All filters are optional and
    /// combined with AND; --email and --username match "contains", case-insensitive.
    /// Without filters, lists every user the identity is allowed to read (requires
    /// the user.read permission, e.g. `ORG_OWNER` or `ORG_USER_MANAGER` on the user's
    /// organization). Pagination: --limit/--offset; the response's
    /// details.totalResult is the total number of matches (absent when there are
    /// none). Each result has userId, username, state, details.resourceOwner (its
    /// organization) and either a "human" (profile, email, phone) or a "machine"
    /// object. --select (or --select-all) is required.
    #[command(after_help = "Examples:\n  zitadel user search --email @acme.com --select result.userId,result.username,result.human.email.email\n  zitadel user search --username john --state active --select result.userId,result.state\n  zitadel user search --organization-id 123456789 --limit 50 --offset 50 --select details.totalResult,result.userId")]
    Search {
        /// Email contains this text (case-insensitive), e.g. "@acme.com" or a full address
        #[arg(long)]
        email: Option<String>,
        /// Username contains this text (case-insensitive)
        #[arg(long)]
        username: Option<String>,
        /// Only users in this state
        #[arg(long, value_enum)]
        state: Option<UserState>,
        /// Only users belonging to this organization id
        #[arg(long)]
        organization_id: Option<String>,
        /// Maximum number of results to return
        #[arg(long, default_value_t = 100, value_parser = clap::value_parser!(u32).range(1..))]
        limit: u32,
        /// Number of results to skip (for paging through results)
        #[arg(long, default_value_t = 0)]
        offset: u64,
    },
    /// Get one user by id
    ///
    /// Calls ZITADEL's v2 `GetUserByID` (GET /v2/users/<user-id>). Returns
    /// details plus user: userId, username, state, loginNames, details.resourceOwner
    /// (its organization) and a "human" (profile, email, phone) or "machine" object.
    /// Always prints the full response regardless of --select — it is a single
    /// object (an explicit --select is still honored). Use user search to find ids.
    #[command(after_help = "Examples:\n  zitadel user get 123456789012345678\n  zitadel user get 123456789012345678 --select user.username,user.state,user.human.email.email")]
    Get {
        /// The user's id (`userId` in user search results)
        user_id: String,
    },
}

#[derive(Debug, Subcommand)]
pub enum OrganizationCommand {
    /// List the organizations the identity may read
    ///
    /// Calls ZITADEL's v2 `ListOrganizations` (POST /v2/organizations/_search).
    /// Visibility depends on the identity's roles: an instance administrator
    /// (`IAM_OWNER`) sees every organization, an `ORG_OWNER` only its own. Each result
    /// has id, name, state, primaryDomain. Pagination: --limit/--offset; the
    /// response's details.totalResult is the total number of matches (absent when
    /// there are none). --select (or --select-all) is required.
    #[command(after_help = "Examples:\n  zitadel organization list --select result.id,result.name,result.state\n  zitadel organization list --name acme --select result.id,result.name")]
    List {
        /// Name contains this text (case-insensitive)
        #[arg(long)]
        name: Option<String>,
        /// Maximum number of results to return
        #[arg(long, default_value_t = 100, value_parser = clap::value_parser!(u32).range(1..))]
        limit: u32,
        /// Number of results to skip (for paging through results)
        #[arg(long, default_value_t = 0)]
        offset: u64,
    },
}

#[derive(Debug, Subcommand)]
pub enum ProjectCommand {
    /// List the projects the identity may read
    ///
    /// Calls ZITADEL's v2 `ListProjects` (`ProjectService`). Filters are optional and
    /// combined with AND. Requires the project.read permission (e.g. `ORG_OWNER` or
    /// `PROJECT_OWNER`); only projects the identity may read are returned. Each
    /// project has projectId, name, state, organizationId, creationDate,
    /// changeDate. Pagination: --limit/--offset; the response's
    /// pagination.totalResult is the total number of matches (absent when there are
    /// none). --select (or --select-all) is required.
    #[command(after_help = "Examples:\n  zitadel project list --select projects.projectId,projects.name\n  zitadel project list --organization-id 123456789 --name app --select pagination.totalResult,projects.projectId,projects.name")]
    List {
        /// Name contains this text (case-insensitive)
        #[arg(long)]
        name: Option<String>,
        /// Only projects owned by this organization id
        #[arg(long)]
        organization_id: Option<String>,
        /// Maximum number of results to return
        #[arg(long, default_value_t = 100, value_parser = clap::value_parser!(u32).range(1..))]
        limit: u32,
        /// Number of results to skip (for paging through results)
        #[arg(long, default_value_t = 0)]
        offset: u64,
    },
}

/// User states accepted by `user search --state` (ZITADEL v2 `UserState`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum UserState {
    Active,
    Inactive,
    Deleted,
    Locked,
    Initial,
}

#[cfg(test)]
#[path = "tests/cli_tests.rs"]
mod tests;
