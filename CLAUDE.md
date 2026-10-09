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
- Each CLI lives as its own crate/binary in the workspace, named after the service it wraps, under `crates/<service>/`. The exceptions are `crates/cli-fields`, `crates/atlassian-auth` and `crates/oauth-user-login`, shared libraries (not binaries, no service of their own) — see the "Shared library: ..." sections below.
- Update this CLAUDE.md, the crate's own CLAUDE.md, and project memory after every significant addition or change — keep them in sync with codebase state.
- When adding a new crate, add a row for it to the table in the root [README.md](README.md).
- To add a new CLI crate from scratch, use the `new-cli-crate` skill (`.claude/skills/new-cli-crate/`). To add a command/subcommand to an existing crate, use `add-cli-command` (`.claude/skills/add-cli-command/`). Both skills open a tracking GitHub issue and a linked branch before writing any code — see "Feature workflow" below.

## Feature workflow: plan → issue → branch

Every feature-sized piece of work (a new crate, a new command, or comparable scope — not a one-line fix, typo, or doc tweak) starts with a GitHub issue, not a branch pulled out of thin air. Issue and branch are linked by construction, not by naming convention alone — this repo's `gh` CLI is authenticated and supports this directly:

1. **Write the plan first.** Concrete: what's being built, the approach, files/commands affected, open questions to confirm with the user.
2. **Create the issue with the plan as its body** — the issue *is* where the plan lives, not a separate doc:
   ```sh
   gh issue create --title "<feature>" --body "<plan>"
   ```
   If the plan changes materially mid-implementation, update the issue (`gh issue edit <number> --body "..."`) rather than letting it go stale.
3. **Create the branch from the issue**, using `gh issue develop` — this both creates the branch and links it to the issue (the link shows up under the issue's "Development" section, and any PR later opened from that branch inherits it automatically):
   ```sh
   gh issue develop <issue-number> --name "issue<issue-number>" --checkout
   ```
   Branch name is always `issue<issue-number>` — no separate slug. The number is what ties branch, issue, and PR together.
4. Implement on that branch as usual (TDD, incremental commits, per the rest of this file).
5. **Independent review, at the end of every plan** — after the last implementation commit and before opening the PR, run the two project subagents in `.claude/agents/` in parallel. Launch them by their own subagent type (never as a fork of the session) and give each **only** the issue number and the branch name — no summary of the work, no hints: their value is a view of the work that doesn't share the session's context.
   - `cold-reviewer` — adversarial code review against the issue: spec match, dead code, edge-case test coverage, correctness bugs, this file's rules; it runs the crate's tests and clippy itself. Fix minor and medium findings yourself and re-run until it approves; stop and ask the user only for severe bugs or findings that need a decision you aren't confident taking alone.
   - `docs-auditor` — documentation vs code in both directions (everything documented is implemented as stated, everything implemented is documented, including `--help`). It assumes neither side is complete or correct and uses the issue as the reference, so a finding may say the **code** is wrong. Fix documentation findings yourself; code findings follow the `cold-reviewer` rules; re-run until it approves.

   Every plan for a new crate or a new command lists this as its last step, and the final report lists each finding with its outcome.
6. **Reference the issue in the PR** — include `Closes #<issue-number>` in the PR body so merging it closes the issue automatically.

Applies to work driven by the `new-cli-crate` and `add-cli-command` skills (see each skill's step 0) and to any other feature-sized request handled outside those skills.

## Tracking known issues and design notes

Known edge cases, deferred fixes, and design notes (documented tradeoffs that aren't scheduled) are tracked as GitHub issues, labeled instead of using an ID prefix per crate:

- **Type** (pick one): `bug`, `enhancement`, `tech-debt` (internal robustness/coverage improvement, not user-facing), `design-note` (a decision already made and documented, not scheduled to change), `needs-verification` (implemented but not confirmed live).
- **Scope** (pick one): the crate name (`jira`, `bitbucket`, `google-chat`, `cli-fields`, `atlassian-auth`, `oauth-user-login`, `atlassian-admin`, `confluence`, `zitadel`) or `cross-crate` for anything spanning multiple crates or repo-wide tooling/CI.

```sh
gh issue create --title "<crate>: <short description>" --label "<type>,<scope>" --body "<what was found, current behaviour, why deferred, what a fix would look like>"
gh issue list --label <scope>   # check existing entries for a crate before adding a new command
```

This is a separate, lighter-weight track from "Feature workflow" above — a backlog note doesn't need a plan or a linked branch, since nobody is implementing it yet.

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

**`--select-all` is capped, not unconditional**: `render_json` (`crates/cli-fields/src/select.rs`) still refuses to print a `Select::All` response once its pretty-printed size exceeds `cli_fields::MAX_ALL_BYTES` (30000 bytes), returning `RenderError::AllTooLarge` with the actual size and top-level fields instead. A caller can't have meaningfully confirmed "fine to flood my own context window" for a response whose size it didn't know in advance — the cap forces a fallback to `--select` once that assumption clearly doesn't hold. This applies uniformly, including to exempt commands' `select.or_all()` (see below) — it's a safety net for the "known to stay small" assumption, not just a guard on the explicit flag.

**Some commands are exempt** and always print their full result regardless of `--select`/`--select-all` (subject to the same `MAX_ALL_BYTES` cap above), via `select.or_all()` at that specific `print_json` call site: commands whose output is either synthesized by the CLI itself (e.g. a delete confirmation) or a single, fixed-shape API response known to stay small (identity checks like `auth whoami`, `doctor`'s internally-generated report, and most single-resource creates/gets/mutations). This is decided **per command, not per crate** — when adding a new command via the `add-cli-command` skill, check whether its output is a list/search (mandatory) or a bounded single object (exempt) and wire `select` vs `select.or_all()` accordingly; see each crate's own CLAUDE.md for its exact classification table.

## Shared library: `crates/atlassian-auth`

OAuth 2.0 authentication against the Atlassian Cloud identity platform (`auth.atlassian.com` / `api.atlassian.com`) is implemented once, in `crates/atlassian-auth` (workspace-local, `path = "../atlassian-auth"` dependency, not published). It provides `AppConfig` (the sectioned `app.json`, one `OAuthConfig` per identity), `Credentials`, `login()` (3LO + PKCE), `login_client_credentials()`, `refresh()`/`renew()`, `load_credentials()`/`save_credentials()`, `renew_rejected()` (after a 401, issue #240), `get_granted_scopes()`, and `cloud_id` resolution via the accessible-resources endpoint. Every function that varies by product (OAuth scopes, config directory name) takes those as parameters rather than hardcoding them — `login(config, scopes)`, `app_config_path(config_dir, cli_dir)` — so each crate's own `auth.rs` becomes a thin wrapper fixing its product-specific values (see `jira`'s `auth.rs` for the pattern; `confluence` follows the same shape).

**Used by**: `jira` and `confluence` — the two crates that actually authenticate against this exact platform with this exact flow (3LO+PKCE / `client_credentials`, `cloud_id` resolution). **Not used by**: `bitbucket` (its own native OAuth consumer — different token endpoint, HTTP Basic auth, no PKCE, no `cloud_id`) or `atlassian-admin` (a static Organization API key, no OAuth grant at all) — both are genuinely different auth models, not further instances of the duplication this library was extracted to eliminate.

## Shared library: `crates/oauth-user-login`

The provider-agnostic half of every human login (`auth login --user`) lives once, in `crates/oauth-user-login` (workspace-local, not published): PKCE (`generate_code_verifier`, `code_challenge`), `generate_state`, and the loopback callback — `bind_listener(addr)` (bind *before* opening the browser, so a busy port fails before the person consents), `wait_for_callback(&listener, callback_path, expected_state)` (answers 404 to stray requests such as `/favicon.ico` and keeps waiting; 400 plus an error on a denial, a missing parameter or a `state` mismatch), and `parse_callback_request_line`. `Identity` (`Service`/`User(UserId)`, `from_user_flag`, `label`, `credentials_path`) names the identities every CLI stores side by side and their credentials files; `UserId` is the caller's id for a person (lowercase slug), and `pending_login_path`, `list_users`, `remove_identity` and `legacy_credentials_files` cover the per-person folders (see "Service and per-person identities" below); `atlassian-auth` re-exports `Identity`. `write_secret_file` (atomic, mode 0600) writes every file holding a secret, and `lock_exclusive` serializes renewals of one credentials file across processes. Its errors are generic and end with a retry instruction; each crate wraps them in its own `LoginError`, appending its exact command (`#[error("{0}: <cli> auth login --user <USER_ID>")]`) unless its `CliError` already adds one (`zitadel`) or it is product-agnostic (`atlassian-auth`). An OAuth redirect on a path other than `callback_path` fails with `CallbackError::WrongPath` naming the expected path, instead of a 404 that would wait forever. `BearerToken<E>` holds the access token a client sends and an optional `Renewer<E>`: `send(send_fn, is_unauthorized)` renews once and repeats a call answered 401 (issue #240), transport-agnostic (each client passes its reqwest closure).

**Remote (two-step) login** (`auth login --user <id> --remote`, issue #146): for a person who is not at the CLI's machine. `PendingLogin::new(redirect_uri, with_pkce, now)` + `save(path)` store `state`, the optional PKCE verifier and the optional redirect URI in that person's `<config_dir>/<cli_dir>/users/<id>/pending-login.json` (mode 0600, valid 10 minutes; one per person, so several people can be mid-login at once); `take_pending_login(path, state, now)` returns it and removes the file (state single-use, consumed before the code exchange; a mismatched state leaves it in place); `pending_login_status` feeds `doctor`'s informational `pending_login` check; `rfc3339_utc` formats `expires_at`. Every crate exposes the same shape: step 1 `--user <id> --remote [--redirect-uri <url>]` prints `{authorize_url, state, expires_at}` (no browser, no port), step 2 `--user <id> --code <code> --state <state>` saves credentials and prints `auth whoami`. Implemented in `zitadel`, `jira`, `confluence` and `bitbucket` (no `--redirect-uri`: Bitbucket always uses the consumer's callback URL); `google-chat` pending (#152).

Provider-specific parts stay in each crate: the authorize URL's parameters, the token endpoint and its auth style (Basic vs public PKCE client), scopes, and refresh semantics. **Used by**: `atlassian-auth` (hence `jira` and `confluence`, whose clients also use `BearerToken` directly), `bitbucket` (state, listener and `BearerToken` — Bitbucket has no PKCE), `google-chat`, `zitadel` (including `BearerToken`), `atlassian-admin` (only `write_secret_file`, for its `app.json`, #218).

## Service and per-person identities (issues #164, #175)

Every CLI with both a non-interactive login and a human login (`jira`, `confluence`, `bitbucket`, `zitadel`) stores the service identity and any number of people side by side, and lets the caller pick one **per call**, with no persisted "active identity". The agent driving these CLIs works for many people and acts as the person it is working for, so each command names that person:

- **Global `--user <USER_ID>` flag** (`#[arg(long, global = true, value_parser = UserId::parse)] user: Option<UserId>` on `Cli`, resolved once in `main.rs` with `Identity::from_user_flag` and passed by reference to every handler). Without it a command acts as the service identity (Atlassian Service Account, Bitbucket OAuth app, ZITADEL service user); with it, as the person who logged in with `auth login --user <USER_ID>`. The id is the caller's own name for the person, a lowercase slug (`a-z`, `0-9`, `.`, `_`, `-`, `:` — e.g. `jane.doe`, `chat:u123` — first character a letter or digit, at most 64): it names a folder, so `..`, `/`, hidden names and case collisions are refused by clap. The value is required: a bare `--user` would silently act as whoever logged in last. The same flag selects what `init`, `auth login`, `auth whoami`, `auth logout` and `doctor` set up, log in, check or remove.
- **One credentials file per identity**: `<config_dir>/<cli>-cli/credentials-service.json` and `<config_dir>/<cli>-cli/users/<id>/credentials.json`, named by `oauth_user_login::Identity::credentials_path`; a person's pending remote login sits next to it (`users/<id>/pending-login.json`). Each login writes only its own file; each file is renewed with its own grant. Credentials of earlier layouts (the pre-#164 `credentials.json`, #164's single `credentials-user.json`) are never read; `doctor` reports them (`identities.legacy_credentials_files`) so they can be deleted. No migration.
- **Renewal under a lock**: `load_credentials` renews an expiring token while holding `oauth_user_login::lock_exclusive` on `<file>.lock`, after reading the file again: Atlassian and Bitbucket rotate refresh tokens, so two parallel calls for the same person must not both spend the same one. A token another call renewed meanwhile is reused. `doctor` renews through the same function.
- **A `401` renews once and repeats the request** (issue #240): a token can be revoked before `expires_at` (the person ended their session, revoked the app), which only the API notices. Every client sends through `oauth_user_login::BearerToken`, holding a renewer that `context::authenticated_client` builds (`rejected_token_renewer`): on a 401 it calls the crate's `renew_rejected(config, path, identity, rejected_token)` — same lock; if the file no longer holds the rejected token another call already renewed it and that token is used — and sends the rebuilt request once more (safe for a POST: a 401 means nothing was processed). A refused renewal travels as `ClientError::Renewal(Box<CliError>)`, already mapped by `login_error_to_cli` (a person's refused refresh is `UserLoginExpired`, exit 3), and `client_error_to_cli` passes it through unchanged; a 401 with the fresh token is reported as before. `doctor`'s `api` check uses the same client, and a failed renewal there fails the `credentials` check instead.
- **Typed secrets are never echoed** (issue #196): `jira`, `confluence` and `bitbucket` `init` read the client secret with `rpassword` (echo off) when stdin is a terminal and as a plain line when it is piped, so scripts keep working (`read_secret_with` injects the terminal check and both readers for tests); an empty answer to any `init` prompt writes nothing (`CliError::EmptyInput`, naming the flag to pass instead). `atlassian-admin init` on a terminal asks for the flags it wasn't given (org id visible, API key hidden; an empty answer writes nothing) and without a terminal writes the skeleton file, never reading the key from stdin (`resolve_credentials`, `read_hidden_with`). `zitadel init` asks the same way (issue #228): the instance URL only when missing, then the service user's whole key JSON (hidden) or the Native app client id.
- **Secret files are owner-only**: credentials, `app.json` (atlassian-admin's too, #218) and the pending login go through `oauth_user_login::write_secret_file` (temporary file renamed over the target, mode 0600).
- **`app.json` with a `service` and a `user` section** (jira/confluence: Service Account + 3LO app; bitbucket: one consumer per identity, possibly the same). The `user` section is the app every person logs in through. `init` / `init --user <id>` rewrite only their own section (the latter also logs that person in); `init --user-app` (issue #195) writes only the `user` section and logs nobody in, for a machine nobody can open a browser on (people then use `auth login --user <id> --remote`), printing just `{"app_config": ...}` for that section. It refuses `--user <id>` at runtime (`CliError::UserAppWithUser`), for the same clap reason as `RemoteLoginNeedsUser`. A legacy flat `app.json` (top-level `client_id`) fails with an error naming both `init` commands. zitadel's `app.json` already held both parts (service user key, Native app `client_id`) and keeps its shape (its `init --user-app` writes it as `init` does, skips every login, and fails without a Native app `client_id`); changing its instance URL discards every stored login.
- **`auth logout [--user <id>]`**: removes that identity's stored login (a person's whole `users/<id>/` folder) and prints `{"logged_out": "<label>"}` (`service` or `user:<id>`); local only, no token revocation. Nothing stored is an error naming the login command.
- **Credentials must match their identity**: renewal picks its grant from the stored token (refresh token or the non-interactive grant), so each crate's `load_credentials(config, path, identity)` — and `doctor` — first runs `check_identity`, refusing a person's file without a refresh token or a service file with one (`LoginError::WrongIdentity`, reported as that identity not being logged in). Otherwise a command could silently act as another identity.
- **Errors name the identity's own command**, with the person's id: a missing file or section, or a failed renewal, says `<cli> auth login` / `<cli> init` for the service identity and `... --user <id>` for a person.
- **"Remote login needs `--user`" is checked in `LoginMode::from_flags`, not by clap**: clap's `requires` cannot see a global flag written before the subcommand (`<cli> --user <id> auth login --code ...`), so `--remote`/`--code` without `--user` returns `CliError::RemoteLoginNeedsUser` at runtime instead. The person's id travels inside `LoginMode` (`UserBrowser(id)`, `RemoteStart { id, .. }`, `RemoteComplete { id, .. }`).
- **Exit code 3 means "this identity needs a new login"** (issue #194, `oauth_user_login::NOT_LOGGED_IN_EXIT_CODE`), so the agent can start a login without parsing the message: each crate's `CliError::exit_code()` returns it for "not logged in" (credentials file missing, of the other identity, or — zitadel — corrupted) and for `CliError::UserLoginExpired` (a person's refresh token refused, also when renewing after a 401 — see above). A token endpoint answering 400/401/403 is `LoginError::TokenRejected` (`oauth_user_login::token_request_rejected`) unless its error is `invalid_client` (app.json's app refused: a new login through it would fail too); that and anything else (network, 408, 429, 5xx) stays `TokenExchange` and exits 1, since retrying may work. A refused renewal of the service identity also exits 1 (its grant is app.json's own credentials), and so do an unreadable (not missing) credentials file (`IoError`), `doctor` and every other error; clap's usage errors exit 2.
- **`doctor`** runs its check cascade for the selected identity; `pending_login` is the selected person's (always `none` for the service identity); the informational `identities` check (`selected`, `service` present/missing, `users` with the ids logged in, `legacy_credentials_files`) stays outside `all_ok`.

A new command that calls the service API takes `&Identity` and passes it to `context::authenticated_client(identity)`; nothing else about identities belongs in a command handler.

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
- `main()` returns `ExitCode`; `run()` returns `Result<(), CliError>`; a single `match run()` in `main` prints the error and returns the exit code (`CliError::exit_code()` where a crate defines it — see "Service and per-person identities" for code 3 — otherwise 1). No `std::process::exit` anywhere.

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
- `.github/workflows/release-pr.yml` + `cliff.toml` — versioning and changelogs are computed entirely from git history via [git-cliff](https://github.com/orhun/git-cliff) (no crates.io registry lookup involved) and applied via [cargo-release](https://github.com/crate-ci/cargo-release). On every push to `main`, for each of `jira`/`bitbucket`/`google-chat` independently: the workflow first checks whether any commit since that crate's last tag (scoped to `crates/<crate>/**`) is a `feat`/`fix`/`perf` or breaking-change conventional commit — git-cliff itself has no config to exclude commit types from its own bump computation, so this gate is a `grep` the workflow runs *before* git-cliff is ever invoked (see `cliff.toml`'s trailing comment); `docs`/`chore`/`ci`/`refactor`/`style`/`test`-only commits don't trigger a release even if they touch the crate's files. If the gate passes, a separate step calls `git-cliff --bumped-version` directly to compute the next semver version, then `cargo release <version> -p <crate> --no-confirm --no-publish --no-tag --no-push --execute` bumps `crates/<crate>/Cargo.toml` and re-invokes git-cliff as a `pre-release-hook` (from that crate's own `[package.metadata.release]` in its `Cargo.toml`) to prepend the changelog entry for that already-decided version. The result is committed and force-pushed to a single stable branch `release/<crate>` (reused and reset every run — never a new dated branch) with a PR opened/updated in place. No `cargo publish` (each of jira/bitbucket/google-chat declares `publish = false` directly in its own `[package]`; these are internal CLIs, not libraries).
- `.github/workflows/release-tag.yml` — also runs on every push to `main` (so it fires again once a release PR merges): for each crate, compares its current `Cargo.toml` version against existing git tags matching that crate's pattern, and creates+pushes the tag (`<crate>-v<version>`) if missing.
- `.github/workflows/release.yml` — unchanged by the git-cliff/cargo-release migration, triggered by the `<crate>-v<version>` tag `release-tag.yml` creates; builds the release binary for that one crate and attaches it to the GitHub Release. Matrix build across three native runners (no cross-compilation): `ubuntu-latest` (linux-x86_64), `ubuntu-24.04-arm` (linux-arm64), `macos-latest` (macos-arm64, Apple Silicon).
- **Why `release-pr.yml`/`release-tag.yml` use `secrets.RELEASE_PLZ_TOKEN` instead of the default `GITHUB_TOKEN`**: GitHub Actions does not trigger other workflows for pushes/tags/PRs made with the default `GITHUB_TOKEN` (anti-recursion guard). Without a PAT, `release-tag.yml`'s tag push would silently never trigger `release.yml`, and `release-pr.yml`'s PR wouldn't trigger `ci.yml` — confirmed by testing this exact failure live under release-plz, the tool this pipeline replaced. `RELEASE_PLZ_TOKEN` (name kept as-is from the release-plz era — renaming a GitHub secret requires recreating the value, not worth the busywork) is a PAT with `Contents: Read and write` and `Pull requests: Read and write` on this repo.
- **Commit scope is still load-bearing**: commit messages must use the affected crate as the conventional-commit scope (`feat(jira): ...`, `fix(bitbucket): ...`) and touch that crate's files — git-cliff's `--include-path "crates/<crate>/**"` attributes commits to a crate by which files it touches (same principle release-plz used), and `feat`/`fix`/breaking-change prefixes drive the computed bump level.
- **`release/<crate>` branches auto-delete on merge** — the repo has `delete-branch-on-merge` enabled specifically so these bot-managed branches never linger as stale clutter requiring manual cleanup (as happened repeatedly under release-plz's dated `release-plz-YYYY-MM-DD...` branches).
