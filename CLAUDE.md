# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project purpose

Monorepo: single Cargo workspace holding many CLI tools, one per external service. All LLM-facing CLI projects live here together — this repo is the workspace root, not a per-crate repo. Goal: replace MCP servers — give an LLM a CLI binary instead of an MCP integration. **Every design decision must optimize for LLM usage**, not human usage:

- Output should be easy for an LLM to parse (prefer structured/predictable text or JSON over decorative human formatting).
- Errors must be clear and actionable for an LLM to self-correct (what went wrong, what to do next).
- `--help` is mandatory on every CLI and every subcommand — it is the LLM's primary discovery mechanism, so keep it accurate and complete.
- Command and flag names should be unambiguous and self-describing; avoid abbreviations an LLM would have to guess at.

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

- `cli.rs` — clap derive arg parsing (`Cli` → `Command` → subcommands per resource, e.g. `issue get <KEY>`). Argument-parsing tests live here using `Cli::try_parse_from`.
- `config.rs` — `Config::from_env()` reads credentials from environment variables: `JIRA_BASE_URL`, `JIRA_EMAIL`, `JIRA_API_TOKEN`. Tested via an injectable getter function (`from_getter`) so no real env access is needed in tests.
- `client.rs` — `JiraClient` wraps a blocking `reqwest` client, does HTTP basic auth against the Jira REST API (`/rest/api/3/...`), and returns raw `serde_json::Value`.
- `main.rs` — wires CLI → config → client, prints results as pretty-printed JSON to stdout, errors to stderr with non-zero exit. Output format favors LLM consumption (structured JSON, actionable error hints).

Credentials are environment-variable based by design choice (simplicity over persistent config files).
