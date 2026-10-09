# CLAUDE.md — crates/bitbucket

Architecture and design notes for the `bitbucket` crate. Global rules (TDD, error handling, flag conventions, commands) are in the root `CLAUDE.md`.

## Status

`init`, `doctor`, `auth login`, `auth whoami`, `auth logout`, `repo get`, `repo list`, `repo create`, `repo delete`, `pr get`, `pr list`, `pr create`, `pr update`, `pr comment`, `pr list-comments`, `pr update-comment`, `pr approve`, `pr unapprove`, `pr decline`, `pr merge`, `pr diff`, `branch list`, `branch create`, `branch suggest-name`, `workspace members` implemented. Other commands not started yet.

## Module map (mirrors crates/jira)

```
src/
  commands/
    mod.rs        — pub mod declarations for all command handlers
    auth.rs       — run_login(), run_whoami(), run_logout() [implemented]
    doctor.rs     — run_doctor(); also called by init as final verification [implemented]
    init.rs       — run_init(), run_init_user_app(), write_app_config(AppSection),
                    user_app_check(), check_user_app_flag(), prompt_secret()
                    (hidden on a terminal, #196), non_empty() (an empty answer is
                    CliError::EmptyInput, #218); human onboarding flow [implemented]
    repo.rs       — run(RepoCommand); dispatches all repo subcommands   [get, list, create, delete implemented]
    pr.rs         — run(PrCommand); dispatches all pr subcommands       [get, list, create, update, comment,
                    list-comments, update-comment, approve, unapprove, decline, merge, diff implemented]
    branch.rs     — run(BranchCommand); dispatches all branch subcommands [list, create, suggest-name implemented]
    workspace.rs  — run(WorkspaceCommand); dispatches all workspace subcommands [members implemented]
  auth.rs         — AppConfig (sectioned app.json), OAuthConfig, Credentials,
                    Identity/UserId (re-exported from oauth-user-login), credentials_path(dir,
                    identity), pending_login_path(dir, id), list_users(),
                    legacy_credentials_files(), remove_identity(), login_client_credentials(),
                    login() (authorization_code, --user <id>), renew(), load_credentials()
                    (renews under a per-file lock)/save_credentials() (0600), renew_rejected()
                    (after a 401, issue #240) [implemented];
                    state, the callback listener and the secret-file helpers come from
                    crates/oauth-user-login
  client.rs       — BitbucketClient (blocking reqwest); get_json/post_json/put_json/delete helpers over
                    send(), which sends through oauth_user_login::BearerToken (with_renewer: a 401
                    renews once and repeats the request, issue #240; ClientError::Renewal);
                    Bitbucket REST API v2.0 methods [get_current_user, get_repository,
                    list_repositories, create_repository, delete_repository, list_pull_requests,
                    get_pull_request, create_pull_request, update_pull_request,
                    create_pull_request_comment, list_pull_request_comments,
                    update_pull_request_comment, approve_pull_request, unapprove_pull_request,
                    decline_pull_request, merge_pull_request, get_pull_request_diff, list_branches,
                    list_workspace_members implemented]
  cli.rs          — clap structs: Cli (--select, --select-all, --user <USER_ID> global), Command, AuthCommand, RepoCommand,
                    PrCommand, BranchCommand, WorkspaceCommand. No logic.
  context.rs      — config_dir(), load_app_config(), load_oauth_config(identity),
                    authenticated_client(identity) (client with rejected_token_renewer, issue #240),
                    client_error_to_cli(e) (Renewal as is, else ApiRequestFailed), login_command(identity),
                    login_error_to_cli(e, identity), oauth_section(), app_config_error(),
                    print_json(value, select), split_repository(repository) (shared by
                    repo and pr commands).
  endpoints.rs    — URL/path constants for OAuth and REST API v2.0.
  error.rs        — CliError (top-level, thiserror-derived), including a
                    transparent Select variant wrapping cli_fields::RenderError.
  tests/          — all *_tests.rs files, mirroring the src/ layout (see "Test file
                    convention" below). tests/e2e_tests.rs holds the ignored e2e tests
                    against a real workspace (see "Testing" below).
                    tests/test_support.rs = a local HTTP server for client_tests.rs (the
                    retry after a 401, issue #240) and doctor's revoked-token tests;
                    client.rs has a test-only with_base_url() seam for it.
  main.rs         — pure dispatch: resolve --select/--select-all into a
                    cli_fields::Select and --user <USER_ID> into an Identity once, match
                    Command, call commands::*.
```

`--select` dot-notation projection itself (`filter_fields`, `describe_top_level_shape`, the `Select` enum, `render_json`) lives in the shared `crates/cli-fields` workspace crate, not in this crate — see root `CLAUDE.md`'s "Shared library: crates/cli-fields".

## Test file convention

See root `CLAUDE.md` for the general `src/tests/` convention and the
cli_tests/commands split. In this crate, `auth.rs` and `workspace.rs` are
the thin passthrough modules with no dedicated `tests/commands/` file —
their coverage lives entirely in `cli_tests.rs`. `branch.rs` has a
`tests/commands/branch_tests.rs` covering the pure `suggest-name` helpers
(`infer_kind`, `resolve_prefix_from_branching_model`, `slugify`,
`build_branch_name`); `list`/`create` have no logic to isolate beyond that,
so their coverage stays in `cli_tests.rs`. `context.rs` also has a dedicated
`tests/context_tests.rs`.

## Testing

```sh
# Unit tests (no credentials needed)
cargo test -p bitbucket

# E2e tests (requires `bitbucket auth login`, git on PATH, writable workspace)
cargo test -p bitbucket -- --ignored --test-threads=1

# Recovery: delete orphaned cli-bitbucket-e2e-* repos
cargo test -p bitbucket e2e_cleanup -- --ignored
```

`e2e_pr_lifecycle` creates a throwaway repo (`cli-bitbucket-e2e-pr-<timestamp>`),
pushes branches via `git` over HTTPS (`x-token-auth` + OAuth access token), and
exercises the full pr lifecycle (create/get/list/comment/approve/unapprove/merge/decline)
plus `branch list`. `RepoGuard` deletes the repo on drop. Override the target
workspace with `BITBUCKET_E2E_WORKSPACE` (defaults to `lucabrognaracode`).

## Auth design (implemented)

Decision trail: service accounts can't get scoped API tokens for Bitbucket
(Atlassian limitation — scoped tokens only cover Jira/Confluence/admin APIs), and
Workspace/Repository Access Tokens are Premium-only. The unified
developer.atlassian.com OAuth 2.0 (3LO) app (used for `jira`) also does **not** offer a
Bitbucket API permission to add. So `bitbucket` uses Bitbucket's own **native OAuth
consumer**, with two grants (one consumer per identity, possibly the same one):

- `auth login` (default) — `client_credentials`: no human consent step, no browser,
  no refresh token. Every action is attributed to the OAuth app. **Intentional** for
  bot/agent usage: bot actions must be visibly a bot's.
- `auth login --user <id>` — `authorization_code`: browser consent, callback on
  `127.0.0.1:8080` (the consumer's callback URL must be
  `http://localhost:8080/callback`), stores a `refresh_token`. Every action is
  attributed to the person who consented. For an agent acting as the person it works for.

**The app and any number of people are stored side by side (issues #164, #175)** — see
root `CLAUDE.md`'s "Service and per-person identities". The global `--user <USER_ID>`
flag selects, per call, the app (default: `app.json`'s `service` consumer,
`credentials-service.json`) or that person (`user` consumer, shared by every person,
`users/<id>/credentials.json`); `context::authenticated_client(identity)` loads and
renews (under a per-file lock) only that identity, `LoginMode` carries the person's id
and `LoginMode::identity()` decides which file a login writes; `auth logout` removes
one identity's login. `--remote`/`--code` without `--user` fail in
`LoginMode::from_flags` (`CliError::RemoteLoginNeedsUser`), since clap's `requires`
can't see a global `--user` placed before the subcommand. `init` / `init --user <id>`
write only their own section (`client_credentials` for the app, the browser consent
for that person).
Exit code (issue #194): `CliError::exit_code()` returns 3 for `NotAuthenticatedService`, `NotAuthenticatedUser` and `UserLoginExpired` (a person's refresh token refused, `LoginError::TokenRejected`), 1 for everything else; `context::login_error_to_cli` keeps a refused service renewal, a transient `TokenExchange` and an unreadable (not missing) credentials file (`IoError`) out of code 3 — see root `CLAUDE.md`. `init --user-app` (issue #195) writes only the `user` section through `write_app_config(dir, AppSection::User, ..)`, logs nobody in and prints `commands::init::user_app_check` as `{"app_config": ...}`; `check_user_app_flag` refuses `--user <id>` (`CliError::UserAppWithUser`).

- **OAuth consumer**: created in the Bitbucket workspace (Settings → OAuth consumers →
  Add consumer). Callback URL `http://localhost:8080/callback` is needed only for
  `--user <id>`; it doesn't affect `client_credentials`. Produces a `Key` (client_id) and `Secret`
  (client_secret). The token's identity is whichever account created the consumer —
  in production this should be a dedicated `bot@<domain>` account added as a workspace
  member, not a personal account.
- **Endpoints** (Bitbucket-native, *not* `auth.atlassian.com` / `api.atlassian.com`):
  - Token: `https://bitbucket.org/site/oauth2/access_token` (HTTP Basic auth with
    client_id/client_secret, `grant_type=client_credentials`)
  - API base: `https://api.bitbucket.org/2.0` (workspace slug used directly in paths,
    no `cloud_id` resolution step like jira)
- **Renewal** (60s leeway, see `auth::load_credentials`/`auth::renew`): credentials
  with a `refresh_token` (`--user <id>`) renew via the `refresh_token` grant (Bitbucket
  rotates it on every use; a response without one keeps the old one, so a user
  session never silently becomes the app); without one, via `client_credentials`
  again. Renewal holds a lock on the credentials file and re-reads it first, so
  parallel commands for the same identity renew once. Access tokens last 1 hour; an unused refresh token expires after 3 months.
  A 401 renews once more (`auth::renew_rejected`, same lock) and repeats the request (issue #240):
  a person whose renewal is refused gets exit 3; `doctor` reports it in `credentials`.
- **`--user <id>` specifics** (from Bitbucket's docs): no PKCE and no `redirect_uri`
  parameter — Bitbucket always redirects to the consumer's callback URL. `state` is
  sent and checked for CSRF. The token response may name the scope field `scope` or
  `scopes`; both are accepted.
- **`auth login --user <id> --remote` — the same grant in two steps** (issue #146), for a
  person not at this machine. Step 1 (`start_remote_login`) saves an
  `oauth_user_login::PendingLogin` with **state only** (no PKCE, no redirect URI) to
  that person's `users/<id>/pending-login.json` (0600, 10 minutes) and prints `{authorize_url, state,
  expires_at}`; step 2 (`--code --state`, `complete_remote_login`) takes it (state
  single-use) and exchanges the code. There is no `--redirect-uri`: Bitbucket always
  redirects to the consumer's callback URL, so remote use needs a consumer whose
  callback URL is the caller's endpoint, in the `user` section of its own config folder. Verified live
  end to end against the localhost consumer (copying `code`/`state` from the
  browser's address bar). `doctor` reports it as `pending_login` (outside `all_ok`).

Config layout, mirroring jira (`$XDG_CONFIG_HOME/bitbucket-cli/`, falling back to
`~/.config/bitbucket-cli/`):

- `app.json` — `{"service": {"client_id": "...", "client_secret": "..."}, "user":
  {"client_id": "...", "client_secret": "..."}}` (each identity's OAuth consumer
  Key/Secret, either section optional). Static, written by `init` / `init --user <id>` / `init --user-app` (`user` only) or by
  hand, mode 0600. The pre-#164 flat shape is rejected with `CliError::AppConfigLegacy`, naming
  both `init` commands; no automatic migration.
- `credentials-service.json` / `users/<id>/credentials.json` — `access_token`, `expires_at`,
  `scopes`, `refresh_token` (only for people). Mode 0600, `<file>.lock` next to them
  during renewals. Fully managed by the CLI. Leftover `credentials.json` (pre-#164) and
  `credentials-user.json` (#164) are not read; `doctor`'s
  `identities.legacy_credentials_files` reports them.
- `users/<id>/pending-login.json` — only between the two steps of `auth login --user <id> --remote`
  (`state`, `expires_at`). Removed by step 2.

## Implemented commands

| Command | Notes |
|---------|-------|
| `init [--user <id>] [--client-id --client-secret]` | Onboarding of one identity (`service` section + `client_credentials`, or `user` section + browser consent for that person); only command with narrative output |
| `init --user-app [--client-id --client-secret]` | Writes only the `user` section, logs nobody in, prints `{"app_config": ...}` for it plus the next login commands; refuses `--user` (issue #195) |
| `doctor [--user <id>]` | Cascading JSON health check for the selected identity (app_config, credentials, api, permissions); `credentials.identity` is `user` or `app`; informational `pending_login` and `identities`; exit non-zero on any failure |
| `auth login [--user <id>]` | default: `client_credentials` exchange (the app) into `credentials-service.json`; `--user <id>`: browser `authorization_code` flow (that person) into `users/<id>/credentials.json` |
| `auth login --user <id> --remote` / `--code --state` | two-step `authorization_code` for someone elsewhere: step 1 prints the consent URL (exempt from `--select`), step 2 exchanges the code and prints `auth whoami` |
| `auth whoami [--user <id>]` | `GET /2.0/user` as the selected identity, supports `--select` |
| `auth logout [--user <id>]` | removes the selected identity's stored login (local only); prints `{"logged_out": ...}` |
| `repo get <workspace>/<repo_slug>` | `GET /2.0/repositories/{workspace}/{repo_slug}`, supports `--select` |
| `repo list <workspace> [--page]` | `GET /2.0/repositories/{workspace}`, paginated (`--page`), supports `--select` |
| `repo create <workspace>/<repo_slug> [--description --private --project]` | `POST /2.0/repositories/{workspace}/{repo_slug}`, `scm` always `git`, supports `--select` |
| `repo delete <workspace>/<repo_slug> --confirm` | `DELETE /2.0/repositories/{workspace}/{repo_slug}`, destructive, requires `--confirm`, synthesizes `{"deleted": true, "repository": ...}`, supports `--select` |
| `pr list <workspace>/<repo_slug> [--state --page]` | `GET /2.0/repositories/{workspace}/{repo_slug}/pullrequests`, paginated (`--page`), optional `--state` filter (OPEN/MERGED/DECLINED/SUPERSEDED), supports `--select` |
| `pr get <workspace>/<repo_slug> <id>` | `GET /2.0/repositories/{workspace}/{repo_slug}/pullrequests/{id}`, supports `--select` |
| `pr create <workspace>/<repo_slug> --title --source [--destination --description --close-source-branch --reviewers --draft]` | `POST /2.0/repositories/{workspace}/{repo_slug}/pullrequests`, `--reviewers` is a comma-separated list of reviewer UUIDs (find them with `workspace members`), `--draft` sends `"draft": true`, supports `--select` |
| `pr update <workspace>/<repo_slug> <id> [--title --description --destination --reviewers --draft --ready-for-review]` | `PUT /2.0/repositories/{workspace}/{repo_slug}/pullrequests/{id}`, pull request must be open, at least one flag required, `--reviewers` replaces the full reviewer list (not additive, same format as `pr create --reviewers`), `--draft`/`--ready-for-review` are mutually exclusive and send `"draft": true`/`"draft": false`, supports `--select` |
| `pr comment <workspace>/<repo_slug> <id> --content [--path --line \| --parent]` | `POST /2.0/repositories/{workspace}/{repo_slug}/pullrequests/{id}/comments`, `--path`/`--line` for inline comments (both or neither), `--parent <comment_id>` for a reply (sends `"parent": {"id": N}`, conflicts with `--path`/`--line`), supports `--select` |
| `pr list-comments <workspace>/<repo_slug> <id> [--page]` | `GET /2.0/repositories/{workspace}/{repo_slug}/pullrequests/{id}/comments`, paginated (`--page`), returns all comments including deleted ones (`deleted: true`) and replies, no client-side filtering, supports `--select` |
| `pr update-comment <workspace>/<repo_slug> <id> <comment_id> --content` | `PUT /2.0/repositories/{workspace}/{repo_slug}/pullrequests/{id}/comments/{comment_id}`, replaces the comment text only (inline position unchanged), usually author-only on Bitbucket's side, supports `--select` |
| `pr approve <workspace>/<repo_slug> <id>` | `POST /2.0/repositories/{workspace}/{repo_slug}/pullrequests/{id}/approve`, supports `--select` |
| `pr unapprove <workspace>/<repo_slug> <id>` | `DELETE /2.0/repositories/{workspace}/{repo_slug}/pullrequests/{id}/approve`, synthesizes `{"unapproved": true, "id": ...}`, supports `--select` |
| `pr decline <workspace>/<repo_slug> <id> --confirm` | `POST /2.0/repositories/{workspace}/{repo_slug}/pullrequests/{id}/decline`, destructive, requires `--confirm`, supports `--select` |
| `pr merge <workspace>/<repo_slug> <id> --confirm [--message --merge-strategy --close-source-branch]` | `POST /2.0/repositories/{workspace}/{repo_slug}/pullrequests/{id}/merge`, destructive, requires `--confirm`, supports `--select` |
| `pr diff <workspace>/<repo_slug> <id> [--context --path]` | `GET /2.0/repositories/{workspace}/{repo_slug}/pullrequests/{id}/diff`, raw unified diff text (not JSON), `--select` has no effect |
| `branch list <workspace>/<repo_slug> [--page]` | `GET /2.0/repositories/{workspace}/{repo_slug}/refs/branches`, paginated (`--page`), supports `--select` |
| `branch create <workspace>/<repo_slug> <name> --target` | `POST /2.0/repositories/{workspace}/{repo_slug}/refs/branches`, `--target` is a branch name or commit hash to create the new branch from, supports `--select` |
| `branch suggest-name --issue-key --issue-type --issue-summary [--repository --prefix]` | Computes a suggested branch name from a Jira issue; `--prefix` overrides, else `--repository` resolves the real prefix via `GET /2.0/repositories/{workspace}/{repo_slug}/branching-model` (requires auth), else falls back to a local Bug→bugfix/else→feature heuristic (no auth), supports `--select` |
| `workspace members <workspace> [--page]` | `GET /2.0/workspaces/{workspace}/members`, paginated (`--page`), supports `--select`; the way to resolve a person to the `uuid` needed by `pr create --reviewers` |

`doctor`/`init` are duplicated from jira's pattern (see "Future: shared Atlassian
library" below). Unlike jira (which calls `/rest/api/3/mypermissions` and reports a
fixed map of permission booleans), Bitbucket's token response already includes the
granted `scopes` — `auth::Credentials` persists them, and `doctor`'s `permissions`
check reports them as-is (`granted_scopes`), no extra API call. `status: "error"`
only if the list is empty (nothing will work); otherwise purely informational —
deliberately not matched against a fixed list of "required" scopes, since which
scopes a command needs is documented per-command, not enforced by `doctor`.

## Planned commands (build incrementally, smallest first)

| Command | Notes |
|---------|-------|
| `pipeline list` / `pipeline get` | CI status, often blocking for merge |

## API design notes

- **`--select`/`--select-all`** (global flags, see root `CLAUDE.md`): `--select` is mandatory by default; omitting both flags fails with the response's byte size and top-level fields instead of printing. `--select-all` is the explicit stateless opt-out. Exempt commands (always print in full via `select.or_all()` at their `print_json` call site) and why:
  | Command | Exempt? | Why |
  |---|---|---|
  | `doctor` | yes | internally-generated report, fixed/small |
  | `auth whoami` | yes | identity check, fixed/small |
  | `auth logout` | yes | synthesized by us: `{"logged_out": "service"}` / `{"logged_out": "user:<id>"}` |
  | `repo get` | yes | single repository object, fixed shape |
  | `repo list` | **no** | paginated collection |
  | `repo create` | yes | single repository object, fixed shape |
  | `repo delete` | yes | synthesized by us: `{"deleted": true, "repository": ...}` |
  | `pr get` | yes | single pull request object, fixed shape |
  | `pr list` | **no** | paginated collection |
  | `pr create` | yes | single pull request object, fixed shape |
  | `pr update` | yes | single pull request object, fixed shape |
  | `pr comment` | yes | single comment object, fixed shape |
  | `pr list-comments` | **no** | paginated collection |
  | `pr update-comment` | yes | single comment object, fixed shape |
  | `pr approve` | yes | small approval object |
  | `pr unapprove` | yes | synthesized by us: `{"unapproved": true, "id": ...}` |
  | `pr decline` | yes | single pull request object, fixed shape |
  | `pr merge` | yes | single pull request object, fixed shape |
  | `pr diff` | N/A | raw diff text, not JSON, `--select` has no effect |
  | `branch list` | **no** | paginated collection |
  | `branch create` | yes | single branch object, fixed shape |
  | `branch suggest-name` | yes | small fixed-shape object |
  | `workspace members` | **no** | paginated collection |
- Bitbucket Cloud REST API v2.0 base: `https://api.bitbucket.org/2.0`.
- **Destructive commands** (e.g. `pr merge`, `pr decline`): no interactive prompts; require explicit `--confirm`, error message includes the exact retry command.

## Future: shared Atlassian library

`auth.rs` here duplicates patterns from `crates/jira/src/auth.rs` (config file
layout, `OAuthConfig`/`Credentials`/`LoginError` naming, `now_unix()` helper) but is
Bitbucket-specific (native endpoints, Basic-auth token requests, no PKCE, no `cloud_id`).
Once both crates are stable, consider extracting shared OAuth/config-path code into a
common workspace library — deferred until there is a second real use case to validate
the abstraction.
