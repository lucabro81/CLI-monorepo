---
name: add-bitbucket-command
description: Use when adding a new command or subcommand to the bitbucket CLI crate (crates/bitbucket). Walks through scoping, Bitbucket REST API research, TDD implementation, and a manual live-verification loop until the command works end-to-end against a real Bitbucket workspace, following this repo's conventions.
---

# Adding a command to the `bitbucket` CLI

This skill drives the full lifecycle of adding one command (or subcommand) to
`crates/bitbucket`. Follow the steps in order. Don't skip the research or the
live-verification loop — "compiles and unit tests pass" is not "done" for
this crate.

Read the workspace root `CLAUDE.md` (monorepo-wide rules: TDD, error handling,
flag conventions) and `crates/bitbucket/CLAUDE.md` (module map, auth design,
API design notes, implemented commands table) before starting — both are
short and define conventions this skill assumes.

## 1. Scope the command — ask, don't assume

Before writing anything, make sure these are nailed down. Use AskUserQuestion
for anything ambiguous — guessing wrong here means rework later. This initial
briefing is the most important step: the rest of this skill runs largely
unsupervised, so anything not pinned down here becomes an assumption baked
into the implementation.

Also check `BACKLOG.md` (workspace root) for existing entries related to this
area of the crate (e.g. `REPO-*`, `AUTH-*`, `LIB-1`, `DOCTOR-1`) — a planned
edge case or design note may be directly relevant to the command being added.

- **Command shape**: top-level (`Command` enum) or subcommand of an existing
  group (`RepoCommand`, `AuthCommand`, future `PrCommand`)? Exact name —
  long, descriptive, no abbreviations (per root CLAUDE.md).
- **Inputs**: required vs optional flags, all `#[arg(long)]`, no short
  aliases. Follow the `repo create` precedent (typed flags, one per field)
  unless the field count is large/open-ended (see `REPO-1` for when a raw
  JSON body might make more sense instead).
- **Output**: what JSON shape does it return? Full Bitbucket response, or a
  small synthesized object (e.g. `{"deleted": true, ...}`)?
- **Destructive?** If it deletes/modifies state irreversibly (e.g. `pr
  decline`, `pr merge`, a future `repo delete`), it needs an explicit
  `--confirm` flag — no interactive prompts, an LLM can't respond to those.
  The error message should include the exact retry command.
- **Auth/scope impact**: does this need an OAuth scope not currently granted
  to the consumer? Check `bitbucket doctor`'s `permissions.granted_scopes`
  against Bitbucket's documented scope for the endpoint (see step 2). If a
  new scope is needed, flag this explicitly to the user — it requires adding
  the permission to the OAuth consumer in the workspace (Settings → OAuth
  consumers → edit), a one-time human step they need to be aware of. Do
  **not** add a new key to `doctor`'s permissions check for this — per
  `DOCTOR-1`/the bitbucket `permissions` design, it reports the granted-scopes
  list as-is and is not matched against a fixed "required scopes" list.

## 2. Research the Bitbucket REST API endpoint

Don't guess endpoint paths or payload shapes. For each endpoint involved:

1. Check the official docs at
   `https://developer.atlassian.com/cloud/bitbucket/rest/` (use
   WebFetch/WebSearch) for: HTTP method, path, required/optional params,
   request body shape, response shape, and the **OAuth scope** required.
2. Check whether the response is paginated. Bitbucket Cloud uses page-number
   pagination (`?page=N`, response includes `page`, `pagelen`, `size`, and a
   `next` URL) — see `repo list` / `endpoints::path_repositories` for the
   pattern. This differs from `jira`'s cursor-based `nextPageToken`.
3. Note the exact path string(s)/path-builder function(s) — these go into
   `endpoints.rs`, not inlined in `client.rs`/`auth.rs`.

If the endpoint behavior is uncertain (e.g. exact response field names, or
whether a field is required on create), it's faster to do a quick manual
`curl`/`cargo run` probe against a real test workspace than to keep re-reading
docs. Ask the user for a workspace/repo slug to test against if one isn't
already established in the conversation (e.g. `lucabrognaracode/repo-test`,
`lucabrognaracode/cli-test-repo`).

## 3. Design — write it down before coding

Sketch (briefly, in your own response, not a separate doc):

- The `endpoints.rs` constant(s)/path-builder function(s) needed.
- The `client.rs` method signature (return type: raw `serde_json::Value`
  unless there's a strong reason for a typed struct, per existing pattern;
  use the existing `get_json`/`post_json` private helpers, adding a new one
  — e.g. `put_json`/`delete_json` — only if genuinely needed).
- Any new `ClientError`/`CliError` variants — `thiserror`, message = problem +
  corrective action, no `unwrap`/`expect` outside tests.
- The `cli.rs` struct/variant, with `after_help` examples if the command has
  multiple meaningful flag combinations.
- The `commands/<group>.rs` handler function signature, and any pure helper
  functions (e.g. `build_create_body`, `split_repository`) that should be
  unit-tested in isolation from the network call.

Then implement **incrementally, one logical unit at a time** (per root
CLAUDE.md): e.g. endpoints → client method → error variants → cli parsing →
handler → dispatch, each as its own edit with a one-line description of what
it does and why, so the pieces remain individually reviewable even though the
loop runs end-to-end without pausing for approval between them.

## 4. TDD — red, then green

Per root CLAUDE.md, tests must exist and fail before implementation:

1. **`cli_tests.rs`**: write parsing tests for the new command/flags first
   (e.g. `parses_repo_<new>_with_no_optional_flags`,
   `parses_repo_<new>_with_all_flags`). Run `cargo test -p bitbucket` —
   confirm these fail to compile or fail because the variant doesn't exist
   yet.
2. If the command involves new client-side logic beyond a passthrough HTTP
   call (e.g. body construction like `build_create_body`, response
   reshaping, identifier splitting like `split_repository`) — write unit
   tests for that logic first too.
3. **Review the tests as a senior engineer would**, before implementing
   anything (per root CLAUDE.md): are edge cases covered, are assertions
   meaningful (exact-output where practical, not just "doesn't crash"), do
   they reflect realistic LLM-agent usage? Revise the tests until satisfied —
   only then move on.
4. If a new `<module>_tests.rs` file is created, it needs
   `#![allow(clippy::unwrap_used, clippy::expect_used)]` at the top (test
   files are exempt from the workspace-wide deny on those lints).
5. Implement the minimum code to make these pass: `endpoints.rs` →
   `client.rs` → `error.rs` (if needed) → `cli.rs` → `commands/<group>.rs` →
   `main.rs` dispatch (often already a generic passthrough — check before
   adding a new match arm).
6. `cargo test -p bitbucket` — all green. `cargo clippy -p bitbucket
   --all-targets` — zero warnings (fix `pedantic` lints too, don't `#[allow]`
   them away unless there's a real reason).

## 5. Manual live-verification loop

There is no automated e2e suite for this crate (unlike `jira`). Verification
is manual, against a real Bitbucket workspace:

```sh
cargo run -p bitbucket -- <command> --help          # accurate, complete help text?
cargo run -p bitbucket -- <command> ...             # against a real workspace
```

Iterate here against the live API until the output looks right — this is
where wrong endpoint assumptions from step 2 usually surface (e.g. an
unexpected 400 because of a workspace-level constraint, a field name that
differs from the docs).

If the command is destructive or creates persistent state (e.g. `repo
create`), tell the user what was created/modified in the real workspace as
part of the final report (step 7) — don't silently leave test artifacts
without mentioning them.

## 6. Verification loop — run until it actually works

Repeat until everything below is green, fixing root causes (not loosening
assertions or adding `--allow` to silence problems):

```sh
cargo test -p bitbucket
cargo clippy -p bitbucket --all-targets
```

If a live test fails because an assumption from step 2 was wrong (wrong
endpoint, wrong scope, wrong response field name), go back to step 2, correct
it, and re-run the full loop — don't patch around it locally.

## 7. Docs and commit

- `crates/bitbucket/README.md`: add a `### bitbucket <command>` section with
  usage, flags, required scope, and at least one example, following the style
  of existing entries.
- `crates/bitbucket/CLAUDE.md`: update the "Status" line, module map
  annotations, and the "Implemented commands"/"Planned commands" tables.
  Update "API design notes" if this introduced a new pattern (new pagination
  style, new HTTP method helper, etc.).
- `BACKLOG.md` (workspace root): note any edge cases discovered but not
  handled, with an ID following the existing `<AREA>-<N>` convention (e.g.
  `REPO-2`).
- Commit in small atomic units per root CLAUDE.md (e.g. one commit for
  client+endpoints+cli+handler+unit tests+docs if it's genuinely one logical
  unit, separate commits for unrelated backlog notes). Each commit message
  ends with `Co-Authored-By: Claude Sonnet 4.6`.

## 8. Final report

This skill is meant to run largely unsupervised end-to-end — possibly invoked
by an agent that is itself a user of this CLI, not a human watching every
step. Compensate for that with a **detailed final report** to the user,
covering:

- What was added (command, flags, endpoint(s), required scope, files touched,
  commits made).
- Every assumption made during step 1 that wasn't explicitly confirmed by the
  user (there shouldn't be many, but name them if they exist).
- Any persistent state created/modified in the live workspace during step 5
  (e.g. a test repository) that the user may want to clean up.
- **Specific points the user should double-check**, called out clearly —
  e.g. "the OAuth scope required couldn't be verified against a real 403
  because the consumer already has broad permissions", "this endpoint's
  pagination behavior was inferred from docs, not observed live with more
  than one page of results". Don't bury these in a wall of text — a short
  bulleted "needs human review" section at the end of the report.
- Final state of the verification loop (test/clippy/live results) and any
  known-skipped checks with a reason.
