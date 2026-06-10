# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project purpose

Monorepo: single Cargo workspace holding many CLI tools, one per external service. All LLM-facing CLI projects live here together — this repo is the workspace root, not a per-crate repo. Goal: replace MCP servers — give an LLM a CLI binary instead of an MCP integration. **Every design decision must optimize for LLM usage**, not human usage:

- Output should be easy for an LLM to parse (prefer structured/predictable text or JSON over decorative human formatting).
- Errors must be clear and actionable for an LLM to self-correct (what went wrong, what to do next).
- `--help` is mandatory on every CLI and every subcommand — it is the LLM's primary discovery mechanism, so keep it accurate and complete.
- Command and flag names should be unambiguous and self-describing; avoid abbreviations an LLM would have to guess at.
- Use only long, descriptive flags (`--page`, `--public`) — no short aliases (`-p`). With clap, this means `#[arg(long)]` without a `short`. Short flags are a keyboard shortcut for humans; for an LLM they're just an extra name to guess and a source of ambiguity (`-p` = `--page`? `--project`? `--public`?).
- For commands that support several meaningful parameter combinations, add one or two concrete examples to their `--help` (clap: `#[command(after_help = "...")]`). An LLM generalizes faster from a worked example than from an abstract parameter description.

## Development approach

- Build CLIs incrementally: start with the smallest useful command set, add new commands only when a concrete need arises. Don't pre-build a full surface area for a service.
- Each CLI lives as its own crate/binary in the workspace, named after the service it wraps, under `crates/<service>/`.
- Update this CLAUDE.md, the crate's own CLAUDE.md, and project memory after every significant addition or change — keep them in sync with codebase state.
- When adding a new crate, add a row for it to the table in the root [README.md](README.md).

## Structure convention for each crate

Every crate follows the same layout:

```
src/
  commands/         ← one module per top-level command group (auth, issue, etc.)
    mod.rs
    <command>.rs
  auth.rs           ← OAuth / auth infrastructure (if applicable)
  client.rs         ← HTTP client for the service API
  cli.rs            ← clap structs only, no logic
  context.rs        ← shared setup helpers (config dir, authenticated client, print_json)
  error.rs          ← CliError (top-level, thiserror-derived)
  fields.rs         ← --select projection (if applicable)
  main.rs           ← pure dispatch, no logic
```

Command handlers live in `commands/`; infrastructure (HTTP client, auth, error types) lives at the crate root. `main.rs` only parses `--select` and dispatches to `commands::*`.

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
- Map internal errors to `CliError` at the `run()` boundary in `main.rs` or in command handlers, not deeper.
- For conditions that are theoretically unreachable (e.g. serializing a well-typed struct), use a dedicated `Internal(String)` variant instead of `unwrap`/`expect`, with a comment explaining why it should never fire.
- `main()` returns `ExitCode`; `run()` returns `Result<(), CliError>`; a single `match run()` in `main` prints the error and returns the exit code. No `std::process::exit` anywhere.

Clippy is configured at workspace level (`[workspace.lints.clippy]` in root `Cargo.toml`) with `unwrap_used`/`expect_used` as `deny` and `pedantic` as `warn`. Each crate opts in with `[lints] workspace = true`. Test modules silence the unwrap/expect denies with `#[allow(clippy::unwrap_used, clippy::expect_used)]` on the `mod tests` block.

## Commands

- Build: `cargo build` (whole workspace) or `cargo build -p <crate>`
- Test: `cargo test -p <crate>`; single test: `cargo test -p <crate> <test_name_substring>`
- Lint: `cargo clippy -p <crate>` — must pass with zero warnings before merging
- Run a CLI: `cargo run -p <crate> -- <args>`, e.g. `cargo run -p jira -- issue get PROJ-123`
- Help: `cargo run -p <crate> -- --help`
