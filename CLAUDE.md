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
- Each CLI lives as its own crate/binary in the workspace, named after the service it wraps, under `crates/<service>/`. The one exception is `crates/cli-fields`, a shared library (not a binary, no service of its own) — see "Shared library: crates/cli-fields" below.
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
  main.rs           ← pure dispatch, no logic
```

Command handlers live in `commands/`; infrastructure (HTTP client, auth, error types) lives at the crate root. `main.rs` resolves `--select`/`--select-all` into a `cli_fields::Select` once and dispatches to `commands::*`. There is no per-crate `fields.rs` anymore — that logic is the shared `cli-fields` crate (see below).

## Shared library: `crates/cli-fields`

`--select` field-projection support used by every crate is implemented once, in `crates/cli-fields` (a workspace-local library, `path = "../cli-fields"` dependency, not published). It provides `Select<'a>` (`Required`/`All`/`Fields(&'a [&'a str])`), `render_json(value, select)`, `filter_fields`, and `describe_top_level_shape`. Each crate's `context::print_json` is a thin wrapper around `cli_fields::render_json`, and each crate's `CliError` has one `#[error(transparent)] Select(#[from] cli_fields::RenderError)` variant.

**`--select` is mandatory by default**: if a command's output could be large or unbounded (search/list endpoints, or — for jira specifically — a single issue, since issues carry arbitrary per-project custom fields), omitting both `--select` and `--select-all` makes the command fail with an error reporting the response's byte size and top-level field names, instead of printing potentially huge JSON that could flood an LLM caller's context window. `--select-all` is the explicit, stateless opt-out (mirrors the `--confirm` pattern already used for destructive commands) — passing it is itself the caller's confirmation that printing the full response is fine.

**Some commands are exempt** and always print their full result regardless of `--select`/`--select-all`, via `select.or_all()` at that specific `print_json` call site: commands whose output is either synthesized by the CLI itself (e.g. a delete confirmation) or a single, fixed-shape API response known to stay small (identity checks like `auth whoami`, `doctor`'s internally-generated report, and most single-resource creates/gets/mutations). This is decided **per command, not per crate** — when adding a new command via the `add-cli-command` skill, check whether its output is a list/search (mandatory) or a bounded single object (exempt) and wire `select` vs `select.or_all()` accordingly; see each crate's own CLAUDE.md for its exact classification table.

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
- Install/update/uninstall prebuilt binaries from GitHub Releases (no clone, no cargo): `scripts/install.sh [install|update|uninstall] [crate...]` — see root README's "Install prebuilt binaries" section.

## CI/CD

- `.github/workflows/ci.yml` — runs on every push/PR to `main`: `cargo build/test/clippy --workspace` as a single quality gate, no per-crate matrix.
- `.github/workflows/release-pr.yml` + `cliff.toml` — versioning and changelogs are computed entirely from git history via [git-cliff](https://github.com/orhun/git-cliff) (no crates.io registry lookup involved — see the historical note below for why that matters) and applied via [cargo-release](https://github.com/crate-ci/cargo-release). On every push to `main`, for each of `jira`/`bitbucket`/`google-chat` independently: if there are new commits since that crate's last tag touching `crates/<crate>/**`, git-cliff computes the next semver version and changelog entry (invoked as a `pre-release-hook` from that crate's own `[package.metadata.release]` in its `Cargo.toml`), cargo-release bumps `crates/<crate>/Cargo.toml`, and the result is committed and force-pushed to a single stable branch `release/<crate>` (reused and reset every run — never a new dated branch) with a PR opened/updated in place. No `cargo publish` (each of jira/bitbucket/google-chat declares `publish = false` directly in its own `[package]`; these are internal CLIs, not libraries).
- `.github/workflows/release-tag.yml` — also runs on every push to `main` (so it fires again once a release PR merges): for each crate, compares its current `Cargo.toml` version against existing git tags matching that crate's pattern, and creates+pushes the tag (`<crate>-v<version>`) if missing.
- `.github/workflows/release.yml` — unchanged by the git-cliff/cargo-release migration, triggered by the `<crate>-v<version>` tag `release-tag.yml` creates; builds the release binary for that one crate and attaches it to the GitHub Release. Matrix build across three native runners (no cross-compilation): `ubuntu-latest` (linux-x86_64), `ubuntu-24.04-arm` (linux-arm64), `macos-latest` (macos-arm64, Apple Silicon).
- **Why `release-pr.yml`/`release-tag.yml` use `secrets.RELEASE_PLZ_TOKEN` instead of the default `GITHUB_TOKEN`**: GitHub Actions does not trigger other workflows for pushes/tags/PRs made with the default `GITHUB_TOKEN` (anti-recursion guard). Without a PAT, `release-tag.yml`'s tag push would silently never trigger `release.yml`, and `release-pr.yml`'s PR wouldn't trigger `ci.yml` — confirmed by testing this exact failure live under release-plz, the tool this pipeline replaced. `RELEASE_PLZ_TOKEN` (name kept as-is from the release-plz era — renaming a GitHub secret requires recreating the value, not worth the busywork) is a PAT with `Contents: Read and write` and `Pull requests: Read and write` on this repo.
- **Commit scope is still load-bearing**: commit messages must use the affected crate as the conventional-commit scope (`feat(jira): ...`, `fix(bitbucket): ...`) and touch that crate's files — git-cliff's `--include-path "crates/<crate>/**"` attributes commits to a crate by which files it touches (same principle release-plz used), and `feat`/`fix`/breaking-change prefixes drive the computed bump level.
- **`release/<crate>` branches auto-delete on merge** — the repo has `delete-branch-on-merge` enabled specifically so these bot-managed branches never linger as stale clutter requiring manual cleanup (as happened repeatedly under release-plz's dated `release-plz-YYYY-MM-DD...` branches).
- **Historical note**: this replaced [release-plz](https://github.com/MarcoIeni/release-plz), which queried crates.io to determine a crate's "currently published version" — for `publish = false` crates that lookup effectively failed, silently disabling version-bump detection after each crate's first release (confirmed 3 times in this repo's history: jira twice, google-chat once, always fixed with a manual `fix(<crate>): bump version...` workaround commit). git-cliff has no registry dependency at all, so this class of bug cannot recur; it also means the "content-free 'chore: release' PR from Cargo.lock drift" annoyance release-plz had is gone too, since `--include-path` excludes commits that only touch the root `Cargo.lock` from any crate's release consideration.
