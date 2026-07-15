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
                    for stdin prompts in init, and a transparent Select variant
                    wrapping cli_fields::RenderError.
  tests/          — all *_tests.rs files, mirroring the src/ layout (see "Test file
                    convention" below). tests/e2e_tests.rs holds the ignored e2e tests.
  main.rs         — pure dispatch: resolve --select/--select-all into a
                    cli_fields::Select once, match Command, call commands::*.
```

`--select` dot-notation projection itself (`filter_fields`, `describe_top_level_shape`, the `Select` enum, `render_json`) lives in the shared `crates/cli-fields` workspace crate, not in this crate — see root `CLAUDE.md`'s "Shared library: crates/cli-fields".

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

See root `CLAUDE.md` for the general `src/tests/` convention and the
cli_tests/commands split. In this crate, `issue.rs` is the thin passthrough
module with no dedicated `tests/commands/` file — its coverage lives entirely
in `cli_tests.rs`.

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
- **`--select`/`--select-all`** (global flags, see root `CLAUDE.md`): `--select` is mandatory by default; omitting both flags fails with the response's byte size and top-level fields instead of printing. `--select-all` is the explicit stateless opt-out. Exempt commands (always print in full via `select.or_all()` at their `print_json` call site) and why:
  | Command | Exempt? | Why |
  |---|---|---|
  | `doctor` | yes | internally-generated report, fixed/small |
  | `auth whoami` | yes | identity check, fixed/small |
  | `issue get` | **no** | issues carry arbitrary per-project custom fields — can be large even for one record |
  | `issue search` | **no** | paginated list, same custom-field risk multiplied |
  | `issue create` | yes | `POST /issue` returns only `{id, key, self}` |
  | `issue delete` | yes | synthesized by us: `{"deleted": true, "key": ...}` |
  | `issue transitions` | yes | bounded workflow-state list, no `expand` requested |
  | `issue transition` | yes | synthesized by us: `{"transitioned": true, ...}` |
  | `issue comment add` | yes | single comment object, fixed shape |
  | `issue comment remove` | yes | synthesized by us: `{"deleted": true, "id": ...}` |
- **`--fields`** (issue search only): server-side Jira field selection. Defaults to `*navigable`. Reduces payload at the source; orthogonal to `--select`.
- **ADF**: comment bodies and issue descriptions are wrapped in Atlassian Document Format by the client methods; callers pass plain text.
- **Destructive commands**: no interactive prompts (an LLM cannot respond). `issue delete` requires explicit `--confirm`; error message includes the exact command to retry. `commands/issue.rs::run` calls `authenticated_client()` **per match arm** rather than once up front, so the `--confirm` check (free, local) runs before the network round-trip a token refresh may require — otherwise a caller who forgot `--confirm` with expired credentials would see a confusing auth error instead of the actionable `DeleteNotConfirmed` one (spotted and fixed alongside `google-chat`'s `messages delete`, which had the same hoisted-auth structure).

## `doctor` permission checks

Three independent layers, each surfaced as its own report key (see
[oauth-scopes-vs-permissions.md](docs/oauth-scopes-vs-permissions.md) for the
conceptual background):

- **`oauth_scopes`** — OAuth scopes granted to the token, from the
  accessible-resources endpoint (`auth::get_granted_scopes`). `error` if empty.
- **`service_user`** — `GET /mypermissions` with no `projectKey`: lists which
  of `PERMISSION_KEYS` are granted *globally*. For project-scoped permission
  keys, Jira evaluates this as "true if true in at least one project" — it can
  be `true` here while `false` for a specific project. `error` if none granted.
- **`projects`** — for every project visible to the account
  (`JiraClient::list_projects`, paginated `/project/search`), reports
  `service_user_permissions` (per-project `GET /mypermissions?projectKey=...`,
  can differ from `service_user`'s global list) and `service_user_roles` (which
  project roles the account belongs to, via `/project/<key>/role` +
  per-role actor lists, matched against the account's `accountId`).
  `status` is based only on `service_user_permissions` (`error` if empty);
  `service_user_roles` is `null` with `service_user_roles_note` explaining why
  if listing roles 401s — that endpoint requires "Administer Projects", which
  the account may not have everywhere. Zero visible projects is itself an
  `error` — an account that can't see any project can't do anything useful.

## Implemented commands

| Command | Notes |
|---------|-------|
| `init [--client-id --client-secret]` | Human onboarding; only command with narrative output |
| `doctor` | Cascading JSON health check (app_config, credentials, api, oauth_scopes, service_user, projects); exit non-zero on any failure |
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
