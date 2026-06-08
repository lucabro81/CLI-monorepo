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

## Commands

- Build: `cargo build` (whole workspace) or `cargo build -p <crate>`
- Test: `cargo test -p <crate>`; single test: `cargo test -p <crate> <test_name_substring>`
- Run a CLI: `cargo run -p <crate> -- <args>`, e.g. `cargo run -p jira -- issue get PROJ-123`
- Help: `cargo run -p <crate> -- --help`

## Architecture

Cargo workspace; each service CLI is its own binary crate under `crates/<service>` (workspace member list in root `Cargo.toml`).

### `crates/jira`

- `cli.rs` — clap derive arg parsing (`Cli` → `Command` → subcommands per resource, e.g. `auth login`, `issue get <KEY>`). Argument-parsing tests use `Cli::try_parse_from`.
- `auth.rs` — OAuth 2.0 (3LO) + PKCE flow against Atlassian:
  - `OAuthConfig` — app-level `client_id`/`client_secret`, loaded from `<config>/jira-cli/app.json` (static, written by hand once)
  - `login()` — full interactive flow: builds the authorization URL (PKCE challenge + CSRF `state`), opens the browser, runs a one-shot local HTTP server on `http://localhost:8080/callback` to catch the redirect, exchanges the code for tokens, resolves the Jira `cloud_id` via the accessible-resources endpoint
  - `Credentials` — `access_token`/`refresh_token`/`expires_at`/`cloud_id`, persisted to `<config>/jira-cli/credentials.json` (dynamic, rewritten by the CLI on login/refresh)
  - `refresh()` / `load_credentials()` — transparent refresh-before-expiry; Atlassian refresh tokens **rotate on every use**, so the stored credentials must be replaced after each refresh
  - Pure/testable building blocks (PKCE generation, URL building, callback request-line parsing, JSON (de)serialization) are unit tested; the full `login()` orchestration (network + browser + TCP server) is exercised manually
- `client.rs` — `JiraClient` wraps a blocking `reqwest` client, authenticates with `Bearer <access_token>` against `https://api.atlassian.com/ex/jira/<cloud_id>/rest/api/3/...`, returns raw `serde_json::Value`.
- `main.rs` — wires CLI → auth (load/refresh credentials, or run login) → client, prints results as pretty-printed JSON to stdout, errors to stderr with non-zero exit and actionable hints (e.g. "run `jira auth login`").

### Config layout (XDG-style, used on every platform for dev/deploy parity)

Both files live under `$XDG_CONFIG_HOME/jira-cli/` (falling back to `~/.config/jira-cli/`) — chosen over OS-specific dirs (e.g. macOS `~/Library/Application Support`) so the same layout works on the Linux machine the agent will eventually run on:

- `app.json` — `{"client_id": "...", "client_secret": "..."}`, the Atlassian OAuth app's static identity (create the app at developer.atlassian.com, OAuth 2.0 (3LO), redirect URI `http://localhost:8080/callback`, scopes `read:jira-work read:jira-user offline_access`)
- `credentials.json` — OAuth tokens, fully managed by the CLI (never edit by hand)

Kept as two separate files so the CLI's automatic writes to `credentials.json` never clobber the hand-written app identity.
