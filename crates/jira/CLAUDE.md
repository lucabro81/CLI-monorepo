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
  auth.rs         — OAuth infrastructure: OAuthConfig, Credentials, login(), refresh(),
                    save_credentials(), load_credentials(), path helpers
  client.rs       — JiraClient (blocking reqwest); get_json/post_json helpers;
                    all Jira API methods: get_issue, get_myself, add_comment,
                    delete_comment, get_transitions, list_transitions_json,
                    apply_transition, create_issue, delete_issue, search_issues
  cli.rs          — clap structs: Cli (--select global), Command, AuthCommand,
                    IssueCommand, CommentCommand. No logic.
  context.rs      — config_dir(), load_oauth_config(), authenticated_client(),
                    print_json(value, select). Shared by all command handlers.
  error.rs        — CliError (top-level, thiserror-derived). Includes IoError
                    for stdin prompts in init.
  fields.rs       — filter_fields(value, select): dot-notation projection,
                    array-aware, backed by FieldTree (recursive BTreeMap).
  main.rs         — pure dispatch: parse --select, match Command, call commands::*.
```

## Test file convention

Tests live in a separate `<module>_tests.rs` file referenced with:
```rust
#[cfg(test)]
#[path = "<module>_tests.rs"]
mod tests;
```
Test files for commands go in `src/commands/` alongside their module. The `#![allow(clippy::unwrap_used, clippy::expect_used)]` attribute goes at the top of each test file.

## OAuth / auth design

- **Flow**: OAuth 2.0 (3LO) + PKCE. `login()` builds the authorization URL, opens the browser, runs a one-shot TCP server on `localhost:8080` for the callback, exchanges the code for tokens, resolves `cloud_id` via the accessible-resources endpoint.
- **Refresh tokens rotate**: Atlassian invalidates the previous refresh token on every use. The new token pair must be written to `credentials.json` immediately after each refresh.
- **Transparent refresh**: `load_credentials()` checks expiry (with a 60s buffer) and refreshes before returning.
- **Scopes**: `read:jira-work read:jira-user write:jira-work offline_access`

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
| `doctor` | Cascading JSON health check; exit non-zero on any failure |
| `auth login` | Interactive OAuth flow |
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
