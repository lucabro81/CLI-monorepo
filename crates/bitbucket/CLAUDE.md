# CLAUDE.md — crates/bitbucket

Architecture and design notes for the `bitbucket` crate. Global rules (TDD, error handling, flag conventions, commands) are in the root `CLAUDE.md`.

## Status

`init`, `doctor`, `auth login`, `auth whoami`, `repo get` implemented. Other commands not started yet.

## Module map (mirrors crates/jira)

```
src/
  commands/
    mod.rs        — pub mod declarations for all command handlers
    auth.rs       — run_login(), run_whoami()      [implemented]
    doctor.rs     — run_doctor(); also called by init as final verification [implemented]
    init.rs       — run_init(), write_app_config(); human onboarding flow [implemented]
    repo.rs       — run(RepoCommand); dispatches all repo subcommands   [get implemented]
    pr.rs         — run(PrCommand); dispatches all pr subcommands       [planned]
  auth.rs         — OAuthConfig, Credentials, login_client_credentials(),
                    load_credentials()/save_credentials() [implemented]
  client.rs       — BitbucketClient (blocking reqwest); get_json helper;
                    Bitbucket REST API v2.0 methods [get_current_user, get_repository implemented]
  cli.rs          — clap structs: Cli (--select global), Command, AuthCommand, RepoCommand.
                    PrCommand to be added later. No logic.
  context.rs      — config_dir(), authenticated_client(), print_json(value, select).
  endpoints.rs    — URL/path constants for OAuth and REST API v2.0.
  error.rs        — CliError (top-level, thiserror-derived).
  fields.rs       — filter_fields(value, select): dot-notation projection (copied from jira).
  main.rs         — pure dispatch: parse --select, match Command, call commands::*.
```

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
| `repo list` | `GET /2.0/repositories/{workspace}` — list repos in a workspace, useful to discover valid slugs |
| `repo create` | `POST /2.0/repositories/{workspace}/{repo_slug}` — write command, not destructive but should log clearly |
| `pr list` | filter by repo, state, author |
| `pr get <id>` | details + diffstat |
| `pr create` | source/dest branch, title, description, reviewers |
| `pr comment` | add comment (general or inline) |
| `pr approve` / `pr decline` | |
| `pr merge` | merge strategy (merge/squash/fast-forward) |
| `pr diff` | for LLM review |
| `branch list` | check existing branches before `pr create` |
| `pipeline list` / `pipeline get` | CI status, often blocking for merge |

## API design notes

- **`--select`** (global flag): client-side dot-notation projection via `fields::filter_fields`, same as jira.
- Bitbucket Cloud REST API v2.0 base: `https://api.bitbucket.org/2.0`.
- **Destructive commands** (e.g. `pr merge`, `pr decline`): no interactive prompts; require explicit `--confirm`, error message includes the exact retry command.

## Future: shared Atlassian library

`auth.rs` here duplicates patterns from `crates/jira/src/auth.rs` (config file
layout, `OAuthConfig`/`Credentials`/`LoginError` naming, `now_unix()` helper) but is
simplified for `client_credentials` (no PKCE, no `refresh_token`, no `cloud_id`).
Once both crates are stable, consider extracting shared OAuth/config-path code into a
common workspace library — deferred until there is a second real use case to validate
the abstraction.
