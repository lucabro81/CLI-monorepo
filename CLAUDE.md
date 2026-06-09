# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project purpose

Monorepo: single Cargo workspace holding many CLI tools, one per external service. All LLM-facing CLI projects live here together — this repo is the workspace root, not a per-crate repo. Goal: replace MCP servers — give an LLM a CLI binary instead of an MCP integration. **Every design decision must optimize for LLM usage**, not human usage:

- Output should be easy for an LLM to parse (prefer structured/predictable text or JSON over decorative human formatting).
- Errors must be clear and actionable for an LLM to self-correct (what went wrong, what to do next).
- `--help` is mandatory on every CLI and every subcommand — it is the LLM's primary discovery mechanism, so keep it accurate and complete.
- Command and flag names should be unambiguous and self-describing; avoid abbreviations an LLM would have to guess at.
- Use only long, descriptive flags (`--page`, `--public`) — no short aliases (`-p`). With clap, this means `#[arg(long)]` without a `short`. Short flags are a keyboard shortcut for humans; for an LLM they're just an extra name to guess and a source of ambiguity (`-p` = `--page`? `--project`? `--public`?).
- For commands that support several meaningful parameter combinations, add one or two concrete examples to their `--help` (clap: `#[command(after_help = "...")]` / `long_about`). An LLM generalizes faster from a worked example than from an abstract parameter description.

## Development approach

- Build CLIs incrementally: start with the smallest useful command set, add new commands only when a concrete need arises. Don't pre-build a full surface area for a service.
- Each CLI lives as its own crate/binary in the workspace, named after the service it wraps.
- Update this CLAUDE.md and project memory after every significant addition or change (new crate, new command, architecture decision) — keep them in sync with codebase state.

## Error handling

Never use `unwrap()` or `expect()` outside `#[cfg(test)]`. Every failure path must produce a typed error that reaches the user as plain text explaining what went wrong and what to do next — no colors, symbols, or formatting (output is read by an LLM).

Define error types with [`thiserror`](https://docs.rs/thiserror):

```rust
#[derive(Debug, thiserror::Error)]
pub enum MyError {
    #[error("what went wrong: {reason}. Do this to fix it: <example>")]
    SomeVariant { reason: String },
}
```

Rules:
- One error enum per module (e.g. `LoginError` in `auth.rs`, `ClientError` in `client.rs`). Top-level `CliError` in `error.rs` is the boundary type that reaches the user.
- `#[error("...")]` strings are self-contained: problem + corrective action in one sentence, plain text only.
- Map internal errors to `CliError` at the `run()` boundary in `main.rs`, not deeper.
- For conditions that are theoretically unreachable (e.g. serializing a well-typed struct), use a dedicated `Internal(String)` variant instead of `unwrap`/`expect`, with a comment explaining why it should never fire.
- `main()` returns `ExitCode`; `run()` returns `Result<(), CliError>`; a single `match run()` in `main` prints the error and returns the exit code. No `std::process::exit` anywhere.

Clippy is configured at workspace level (`[workspace.lints.clippy]` in root `Cargo.toml`) with `unwrap_used`/`expect_used` as `deny` and `pedantic` as `warn`. Each crate opts in with `[lints] workspace = true`. Test modules silence the unwrap/expect denies with `#[allow(clippy::unwrap_used, clippy::expect_used)]` on the `mod tests` block.

## Commands

- Build: `cargo build` (whole workspace) or `cargo build -p <crate>`
- Test: `cargo test -p <crate>`; single test: `cargo test -p <crate> <test_name_substring>`
- Lint: `cargo clippy -p <crate>` — must pass with zero warnings before merging
- Run a CLI: `cargo run -p <crate> -- <args>`, e.g. `cargo run -p jira -- issue get PROJ-123`
- Help: `cargo run -p <crate> -- --help`

## Architecture

Cargo workspace; each service CLI is its own binary crate under `crates/<service>` (workspace member list in root `Cargo.toml`).

### `crates/jira`

- `cli.rs` — clap derive arg parsing (`Cli` → `Command` → subcommands per resource, e.g. `auth login`, `issue get <KEY>`).
- `auth.rs` — OAuth 2.0 (3LO) + PKCE flow against Atlassian:
  - `OAuthConfig` — app-level `client_id`/`client_secret`, loaded from `<config>/jira-cli/app.json` (static, written by hand once)
  - `login()` — full interactive flow: builds the authorization URL (PKCE challenge + CSRF `state`), opens the browser, runs a one-shot local HTTP server on `http://localhost:8080/callback` to catch the redirect, exchanges the code for tokens, resolves the Jira `cloud_id` via the accessible-resources endpoint
  - `Credentials` — `access_token`/`refresh_token`/`expires_at`/`cloud_id`, persisted to `<config>/jira-cli/credentials.json` (dynamic, rewritten by the CLI on login/refresh)
  - `refresh()` / `load_credentials()` — transparent refresh-before-expiry; Atlassian refresh tokens **rotate on every use**, so the stored credentials must be replaced after each refresh
- `client.rs` — `JiraClient` wraps a blocking `reqwest` client, authenticates with `Bearer <access_token>` against `https://api.atlassian.com/ex/jira/<cloud_id>/rest/api/3/...`, returns raw `serde_json::Value`. Private `get_json(path)` and `post_json(path, body)` helpers shared by all methods. DELETE operations build the URL directly.
- `context.rs` — setup helpers used by `run()`: `config_dir()`, `load_oauth_config()`, `authenticated_client()`, `print_json(value, select)`. Centralises the credential-load → refresh → client-build sequence so each command in `main.rs` calls one function. `print_json` applies `--select` projection before printing.
- `error.rs` — `CliError` enum (top-level, `thiserror`-derived). All internal errors are mapped to `CliError` at the `run()` boundary.
- `fields.rs` — `filter_fields(value, select)` applies dot-notation field projection to any `serde_json::Value`. Arrays are projected element-wise automatically. Used by `print_json` when `--select` is set.
- `doctor.rs` — `run_doctor() -> Result<(Value, bool), CliError>`: cascading checks (`app_config` → `credentials` → `api`), each with `status: ok/error/skipped`. Never fails with `CliError` for check failures — all captured in JSON. Used by both `Command::Doctor` and `init::run_init()`.
- `init.rs` — `run_init(client_id, client_secret)`: human onboarding flow. Prints numbered setup instructions, reads client credentials (from flags or stdin prompts), writes `app.json` via `write_app_config()`, runs `auth::login()`, then calls `doctor::run_doctor()` as confirmation.
- `main.rs` — `run() -> Result<(), CliError>` parses `--select` then dispatches to `run_issue()` or handles auth/doctor/init commands inline; `main() -> ExitCode` calls `run()` and prints any error. No logic, no `process::exit`.

#### Test file convention

Tests live in a separate `<module>_tests.rs` file referenced with `#[cfg(test)] #[path = "..."] mod tests;`. This keeps production code files short while retaining access to private items. The `#![allow(clippy::unwrap_used, clippy::expect_used)]` attribute goes at the top of each test file.

### Config layout (XDG-style, used on every platform for dev/deploy parity)

Both files live under `$XDG_CONFIG_HOME/jira-cli/` (falling back to `~/.config/jira-cli/`) — chosen over OS-specific dirs (e.g. macOS `~/Library/Application Support`) so the same layout works on the Linux machine the agent will eventually run on:

- `app.json` — `{"client_id": "...", "client_secret": "..."}`, the Atlassian OAuth app's static identity (create the app at developer.atlassian.com, OAuth 2.0 (3LO), redirect URI `http://localhost:8080/callback`, scopes `read:jira-work read:jira-user write:jira-work offline_access`)
- `credentials.json` — OAuth tokens, fully managed by the CLI (never edit by hand)

Kept as two separate files so the CLI's automatic writes to `credentials.json` never clobber the hand-written app identity.
