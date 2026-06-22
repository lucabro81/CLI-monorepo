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
- To add a new CLI crate from scratch, use the `new-cli-crate` skill (`.claude/skills/new-cli-crate/`). To add a command/subcommand to an existing crate, use `add-cli-command` (`.claude/skills/add-cli-command/`).

## Structure convention for each crate

Every crate follows the same layout:

```
src/
  commands/         ← one module per top-level command group (auth, issue, etc.)
    mod.rs
    <command>.rs
  tests/            ← all *_tests.rs files, mirroring this layout (see "Test
    commands/         file convention" below)
      <command>_tests.rs
    <module>_tests.rs
  auth.rs           ← OAuth / auth infrastructure (if applicable)
  client.rs         ← HTTP client for the service API
  cli.rs            ← clap structs only, no logic
  context.rs        ← shared setup helpers (config dir, authenticated client, print_json)
  endpoints.rs      ← URL/path constants and path-builder functions, no logic
  error.rs          ← CliError (top-level, thiserror-derived)
  fields.rs         ← --select projection (if applicable)
  main.rs           ← pure dispatch, no logic
```

Command handlers live in `commands/`; infrastructure (HTTP client, auth, error types) lives at the crate root. `main.rs` only parses `--select` and dispatches to `commands::*`.

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

`#![allow(clippy::unwrap_used, clippy::expect_used)]` goes at the top of each
test file — they're exempt from the workspace-wide deny on those lints.

Two-level split:
- `tests/cli_tests.rs` — clap parsing tests for every command/subcommand
  (required/optional flags, defaults, rejections). Always present.
- `tests/commands/<module>_tests.rs` — unit tests for non-HTTP logic inside a
  command handler (body builders, validation, identifier splitting). Only
  exists for modules that have such logic to isolate; thin passthrough
  modules have no dedicated file — their coverage lives entirely in
  `cli_tests.rs`.

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

## CI/CD

- `.github/workflows/ci.yml` — runs on every push/PR to `main`: `cargo build/test/clippy --workspace` as a single quality gate, no per-crate matrix.
- `.github/workflows/release-plz.yml` + `release-plz.toml` — versioning and changelogs are handled by [release-plz](https://github.com/MarcoIeni/release-plz), not manually. Each crate is versioned **independently**: merging conventional commits to `main` makes release-plz open/update a "chore: release" PR with the version bump and changelog for the crates that changed; merging that PR creates the git tag (`<crate>-v<version>`) and GitHub Release. No `cargo publish` to crates.io (these are internal CLIs, not libraries — `publish = false` in `release-plz.toml`).
- **Why `release-plz.yml` uses `secrets.RELEASE_PLZ_TOKEN` instead of the default `GITHUB_TOKEN`**: GitHub Actions does not trigger other workflows for pushes/tags made with the default `GITHUB_TOKEN` (anti-recursion guard). Without a PAT, the tag `release-plz-release` creates would silently never trigger `release.yml` — confirmed by testing this exact failure live. `RELEASE_PLZ_TOKEN` is a PAT with `Contents: Read and write` **and** `Pull requests: Read and write` on this repo, stored as a repo secret — `Pull requests` is required by `release-plz-pr` (opens the release PR via the API), `Contents` by `release-plz-release` (pushes tags). Missing `Pull requests` fails with a 403 "Resource not accessible by personal access token" only on the PR-opening job, while the branch push itself still succeeds — confirmed by hitting this exact failure live. If it's ever rotated or removed, tags stop triggering `release.yml` with no visible error in the `release-plz` run itself — check this first if a release tag appears but no binary gets attached.
- `.github/workflows/release.yml` — triggered by the `<crate>-v<version>` tags release-plz creates; builds the release binary for that one crate and attaches it to the Release release-plz already created. Matrix build across three native runners (no cross-compilation): `ubuntu-latest` (linux-x86_64), `ubuntu-24.04-arm` (linux-arm64), `macos-latest` (macos-arm64, Apple Silicon).
- **Commit scope is load-bearing for releases**: commit messages must use the affected crate as the conventional-commit scope (`feat(jira): ...`, `fix(bitbucket): ...`). This is how release-plz attributes a commit to a crate and computes that crate's version bump — an unscoped commit, a wrong scope, or a commit spanning multiple crates without being split produces unreliable changelogs/bumps. If a change truly touches multiple crates, split it into multiple scoped commits.
- **release-plz attributes commits to a crate by which files the commit actually touches, not by the scope text in the message.** A commit with zero file diff (e.g. an empty test commit) is not attributed to any crate by this path-based check, regardless of its scope.
- **Expect occasional "chore: release" PRs with no real changelog content, just `Other: update Cargo.toml dependencies` and a patch bump** — release-plz also reacts to `Cargo.lock` drift from transitive dependency updates, independent of any feat/fix commit. These are harmless; close them without merging if you don't want a content-free release, or merge them if you're fine with the version following the lockfile. Confirmed live: this happened twice in this repo's first week of using release-plz.
