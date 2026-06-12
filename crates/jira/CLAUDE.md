# CLAUDE.md — crates/jira

Architecture and design notes for the `jira` crate. Global rules (TDD, error handling, flag conventions, commands) are in the root `CLAUDE.md`.

## Module map

```
src/
  commands/
    mod.rs        — pub mod declarations for all command handlers
    auth.rs       — run_login(), run_whoami()
    doctor.rs     — run_doctor(); also called by init as final verification
    init.rs       — run_init(), write_app_config(); human onboarding flow
    issue.rs      — run(IssueCommand); dispatches all issue subcommands
  auth.rs         — OAuth infrastructure: OAuthConfig, Credentials, login(),
                    login_client_credentials(), refresh(), renew(),
                    save_credentials(), load_credentials(), path helpers
  client.rs       — JiraClient (blocking reqwest); get_json/post_json helpers;
                    all Jira API methods: get_issue, get_myself, get_my_permissions,
                    add_comment, delete_comment, get_transitions, list_transitions_json,
                    apply_transition, create_issue, delete_issue, search_issues
  cli.rs          — clap structs: Cli (--select global), Command, AuthCommand,
                    IssueCommand, CommentCommand. No logic.
  context.rs      — config_dir(), load_oauth_config(), authenticated_client(),
                    print_json(value, select). Shared by all command handlers.
  endpoints.rs    — URL/path constants for Atlassian OAuth and Jira REST API v3,
                    used by auth.rs and client.rs. No logic.
  error.rs        — CliError (top-level, thiserror-derived). Includes IoError
                    for stdin prompts in init.
  fields.rs       — filter_fields(value, select): dot-notation projection,
                    array-aware, backed by FieldTree (recursive BTreeMap).
  tests/          — all *_tests.rs files, mirroring the src/ layout (see "Test file
                    convention" below). tests/e2e_tests.rs holds the ignored e2e tests.
  main.rs         — pure dispatch: parse --select, match Command, call commands::*.
```

## Running tests

```sh
# Unit tests (no credentials needed)
cargo test -p jira

# E2e tests (requires login + a writable Jira project) — sequential, see README
JIRA_E2E_PROJECT=KAN cargo test -p jira -- --ignored --test-threads=1

# Recovery: delete all [jira-cli-e2e] orphaned issues
JIRA_E2E_PROJECT=KAN cargo test -p jira e2e_cleanup -- --ignored
```

## Test file convention

Test files live under `src/tests/`, mirroring the module they test (e.g.
`src/commands/issue.rs` -> `src/tests/commands/issue_tests.rs`, `src/cli.rs`
-> `src/tests/cli_tests.rs`). Each tested module references its test file with:

```rust
#[cfg(test)]
#[path = "tests/<module>_tests.rs"]              // from src/<module>.rs
#[path = "../tests/commands/<module>_tests.rs"]  // from src/commands/<module>.rs
mod tests;
```

The `#![allow(clippy::unwrap_used, clippy::expect_used)]` attribute goes at
the top of each test file — they're exempt from the workspace-wide deny on
those lints.

## OAuth / auth design

Two grant types, both using `client_id`/`client_secret` from `app.json`:

- **`client_credentials`** (default, `auth login`) — `login_client_credentials()` POSTs `grant_type=client_credentials` + `audience=api.atlassian.com`, no browser. Returns `Credentials` with `refresh_token: None`. Expected mode for agent-driven usage; resulting account has `accountType: "app"`.
- **3LO + PKCE** (`auth login --user`, also used by `init`) — `login()` builds the authorization URL, opens the browser, runs a one-shot TCP server on `localhost:8080` for the callback, exchanges the code for tokens, resolves `cloud_id` via the accessible-resources endpoint. Returns `Credentials` with `refresh_token: Some(...)`.

Both grants resolve `cloud_id` via the accessible-resources endpoint after obtaining the access token.

- **Refresh tokens rotate**: Atlassian invalidates the previous refresh token on every use. The new token pair must be written to `credentials.json` immediately after each refresh.
- **Transparent renewal**: `renew(config, credentials)` dispatches to `refresh()` (if `refresh_token` is `Some`) or re-runs `login_client_credentials()` (if `None`, service account). `refresh()` itself returns `LoginError::Internal` if called with `refresh_token: None`. Both `load_credentials()` and `doctor`'s `check_credentials` go through `renew()` (with a 60s expiry buffer) — never call `refresh()` directly on possibly-expired credentials.
- **Scopes**: `read:jira-work read:jira-user write:jira-work offline_access` (requested by the 3LO authorization URL; `client_credentials` inherits whatever scopes were granted to the app during the 3LO consent).
- The `client_credentials` grant requires the `--user` flow to have been completed at least once for the app to have site access (e.g. via `jira init`).

## Config layout (XDG-style)

Both files live under `$XDG_CONFIG_HOME/jira-cli/` (falling back to `~/.config/jira-cli/`):

- `app.json` — `{"client_id": "...", "client_secret": "..."}`. Static; written by `jira init` or by hand. Never modified at runtime.
- `credentials.json` — OAuth tokens. Fully managed by the CLI; never edit by hand.

Kept separate so automatic token writes never clobber the app identity.

## API design notes

- **Search endpoint**: `GET /rest/api/3/search/jql` (the old `POST /rest/api/3/search` is 410 Gone). Cursor-based pagination via `nextPageToken`.
- **`--select`** (global flag): client-side dot-notation projection via `fields::filter_fields`. Applied by `context::print_json` before printing.
- **`--fields`** (issue search only): server-side Jira field selection. Defaults to `*navigable`. Reduces payload at the source; orthogonal to `--select`.
- **ADF**: comment bodies and issue descriptions are wrapped in Atlassian Document Format by the client methods; callers pass plain text.
- **Destructive commands**: no interactive prompts (an LLM cannot respond). `issue delete` requires explicit `--confirm`; error message includes the exact command to retry.

## Implemented commands

| Command | Notes |
|---------|-------|
| `init [--client-id --client-secret]` | Human onboarding; only command with narrative output |
| `doctor` | Cascading JSON health check (app_config, credentials, api, permissions); exit non-zero on any failure |
| `auth login [--user]` | Default: `client_credentials` (service account, no browser). `--user`: interactive 3LO + PKCE |
| `auth whoami` | GET /myself |
| `issue get <KEY>` | Fetch single issue |
| `issue create` | POST with ADF description/body |
| `issue delete <KEY> --confirm` | Requires explicit confirmation flag |
| `issue transitions <KEY>` | List available workflow transitions |
| `issue transition <KEY> --to <STATUS>` | Case-insensitive match; lists valid options on mismatch |
| `issue search --jql` | Paginated JQL search |
| `issue comment add <KEY> --body` | POST with ADF body |
| `issue comment remove <KEY> <ID>` | DELETE |

## Known edge cases (see BACKLOG.md)

FIELDS-1..4, AUTH-1..2, CREATE-1..2, DELETE-1.
