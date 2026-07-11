# CLAUDE.md — crates/bitbucket

Architecture and design notes for the `bitbucket` crate. Global rules (TDD, error handling, flag conventions, commands) are in the root `CLAUDE.md`.

## Status

`init`, `doctor`, `auth login`, `auth whoami`, `repo get`, `repo list`, `repo create`, `repo delete`, `pr get`, `pr list`, `pr create`, `pr comment`, `pr approve`, `pr unapprove`, `pr decline`, `pr merge`, `pr diff`, `branch list` implemented. Other commands not started yet.

## Module map (mirrors crates/jira)

```
src/
  commands/
    mod.rs        — pub mod declarations for all command handlers
    auth.rs       — run_login(), run_whoami()      [implemented]
    doctor.rs     — run_doctor(); also called by init as final verification [implemented]
    init.rs       — run_init(), write_app_config(); human onboarding flow [implemented]
    repo.rs       — run(RepoCommand); dispatches all repo subcommands   [get, list, create, delete implemented]
    pr.rs         — run(PrCommand); dispatches all pr subcommands       [get, list, create, comment,
                    approve, unapprove, decline, merge, diff implemented]
    branch.rs     — run(BranchCommand); dispatches all branch subcommands [list implemented]
  auth.rs         — OAuthConfig, Credentials, login_client_credentials(),
                    load_credentials()/save_credentials() [implemented]
  client.rs       — BitbucketClient (blocking reqwest); get_json/post_json/delete helpers;
                    Bitbucket REST API v2.0 methods [get_current_user, get_repository,
                    list_repositories, create_repository, delete_repository, list_pull_requests,
                    get_pull_request, create_pull_request, create_pull_request_comment,
                    approve_pull_request, unapprove_pull_request, decline_pull_request,
                    merge_pull_request, get_pull_request_diff, list_branches implemented]
  cli.rs          — clap structs: Cli (--select global), Command, AuthCommand, RepoCommand,
                    PrCommand, BranchCommand. No logic.
  context.rs      — config_dir(), authenticated_client(), print_json(value, select),
                    split_repository(repository) (shared by repo and pr commands).
  endpoints.rs    — URL/path constants for OAuth and REST API v2.0.
  error.rs        — CliError (top-level, thiserror-derived), including a
                    transparent Select variant wrapping cli_fields::RenderError.
  tests/          — all *_tests.rs files, mirroring the src/ layout (see "Test file
                    convention" below). tests/e2e_tests.rs holds the ignored e2e tests
                    against a real workspace (see "Testing" below).
  main.rs         — pure dispatch: resolve --select/--select-all into a
                    cli_fields::Select once, match Command, call commands::*.
```

`--select` dot-notation projection itself (`filter_fields`, `describe_top_level_shape`, the `Select` enum, `render_json`) lives in the shared `crates/cli-fields` workspace crate, not in this crate — see root `CLAUDE.md`'s "Shared library: crates/cli-fields".

## Test file convention

See root `CLAUDE.md` for the general `src/tests/` convention and the
cli_tests/commands split. In this crate, `auth.rs` and `branch.rs` are the
thin passthrough modules with no dedicated `tests/commands/` file — their
coverage lives entirely in `cli_tests.rs`. `context.rs` also has a dedicated
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
consumer** with the `client_credentials` grant — no human consent step, no browser,
no refresh token.

- **OAuth consumer**: created in the Bitbucket workspace (Settings → OAuth consumers →
  Add consumer), **without** a callback URL. Produces a `Key` (client_id) and `Secret`
  (client_secret). The token's identity is whichever account created the consumer —
  in production this should be a dedicated `bot@<domain>` account added as a workspace
  member, not a personal account.
- **Endpoints** (Bitbucket-native, *not* `auth.atlassian.com` / `api.atlassian.com`):
  - Token: `https://bitbucket.org/site/oauth2/access_token` (HTTP Basic auth with
    client_id/client_secret, `grant_type=client_credentials`)
  - API base: `https://api.bitbucket.org/2.0` (workspace slug used directly in paths,
    no `cloud_id` resolution step like jira)
- **Flow**: `client_credentials` grant only. No PKCE, no authorization code, no
  `refresh_token` — the access token is short-lived and is simply re-requested via the
  same exchange when expired (60s leeway, see `auth::load_credentials`).

Config layout, mirroring jira (`$XDG_CONFIG_HOME/bitbucket-cli/`, falling back to
`~/.config/bitbucket-cli/`):

- `app.json` — `{"client_id": "...", "client_secret": "..."}` (the OAuth consumer's
  Key/Secret). Static, written by hand.
- `credentials.json` — `access_token`, `expires_at`. Fully managed by the CLI.

## Implemented commands

| Command | Notes |
|---------|-------|
| `init [--client-id --client-secret]` | Human onboarding; only command with narrative output |
| `doctor` | Cascading JSON health check (app_config, credentials, api, permissions); exit non-zero on any failure |
| `auth login` | runs `client_credentials` exchange, stores `credentials.json` |
| `auth whoami` | `GET /2.0/user`, supports `--select` |
| `repo get <workspace>/<repo_slug>` | `GET /2.0/repositories/{workspace}/{repo_slug}`, supports `--select` |
| `repo list <workspace> [--page]` | `GET /2.0/repositories/{workspace}`, paginated (`--page`), supports `--select` |
| `repo create <workspace>/<repo_slug> [--description --private --project]` | `POST /2.0/repositories/{workspace}/{repo_slug}`, `scm` always `git`, supports `--select` |
| `repo delete <workspace>/<repo_slug> --confirm` | `DELETE /2.0/repositories/{workspace}/{repo_slug}`, destructive, requires `--confirm`, synthesizes `{"deleted": true, "repository": ...}`, supports `--select` |
| `pr list <workspace>/<repo_slug> [--state --page]` | `GET /2.0/repositories/{workspace}/{repo_slug}/pullrequests`, paginated (`--page`), optional `--state` filter (OPEN/MERGED/DECLINED/SUPERSEDED), supports `--select` |
| `pr get <workspace>/<repo_slug> <id>` | `GET /2.0/repositories/{workspace}/{repo_slug}/pullrequests/{id}`, supports `--select` |
| `pr create <workspace>/<repo_slug> --title --source [--destination --description --close-source-branch]` | `POST /2.0/repositories/{workspace}/{repo_slug}/pullrequests`, supports `--select` |
| `pr comment <workspace>/<repo_slug> <id> --content [--path --line]` | `POST /2.0/repositories/{workspace}/{repo_slug}/pullrequests/{id}/comments`, `--path`/`--line` for inline comments (both or neither), supports `--select` |
| `pr approve <workspace>/<repo_slug> <id>` | `POST /2.0/repositories/{workspace}/{repo_slug}/pullrequests/{id}/approve`, supports `--select` |
| `pr unapprove <workspace>/<repo_slug> <id>` | `DELETE /2.0/repositories/{workspace}/{repo_slug}/pullrequests/{id}/approve`, synthesizes `{"unapproved": true, "id": ...}`, supports `--select` |
| `pr decline <workspace>/<repo_slug> <id> --confirm` | `POST /2.0/repositories/{workspace}/{repo_slug}/pullrequests/{id}/decline`, destructive, requires `--confirm`, supports `--select` |
| `pr merge <workspace>/<repo_slug> <id> --confirm [--message --merge-strategy --close-source-branch]` | `POST /2.0/repositories/{workspace}/{repo_slug}/pullrequests/{id}/merge`, destructive, requires `--confirm`, supports `--select` |
| `pr diff <workspace>/<repo_slug> <id> [--context --path]` | `GET /2.0/repositories/{workspace}/{repo_slug}/pullrequests/{id}/diff`, raw unified diff text (not JSON), `--select` has no effect |
| `branch list <workspace>/<repo_slug> [--page]` | `GET /2.0/repositories/{workspace}/{repo_slug}/refs/branches`, paginated (`--page`), supports `--select` |

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
  | `repo get` | yes | single repository object, fixed shape |
  | `repo list` | **no** | paginated collection |
  | `repo create` | yes | single repository object, fixed shape |
  | `repo delete` | yes | synthesized by us: `{"deleted": true, "repository": ...}` |
  | `pr get` | yes | single pull request object, fixed shape |
  | `pr list` | **no** | paginated collection |
  | `pr create` | yes | single pull request object, fixed shape |
  | `pr comment` | yes | single comment object, fixed shape |
  | `pr approve` | yes | small approval object |
  | `pr unapprove` | yes | synthesized by us: `{"unapproved": true, "id": ...}` |
  | `pr decline` | yes | single pull request object, fixed shape |
  | `pr merge` | yes | single pull request object, fixed shape |
  | `pr diff` | N/A | raw diff text, not JSON, `--select` has no effect |
  | `branch list` | **no** | paginated collection |
- Bitbucket Cloud REST API v2.0 base: `https://api.bitbucket.org/2.0`.
- **Destructive commands** (e.g. `pr merge`, `pr decline`): no interactive prompts; require explicit `--confirm`, error message includes the exact retry command.

## Future: shared Atlassian library

`auth.rs` here duplicates patterns from `crates/jira/src/auth.rs` (config file
layout, `OAuthConfig`/`Credentials`/`LoginError` naming, `now_unix()` helper) but is
simplified for `client_credentials` (no PKCE, no `refresh_token`, no `cloud_id`).
Once both crates are stable, consider extracting shared OAuth/config-path code into a
common workspace library — deferred until there is a second real use case to validate
the abstraction.
