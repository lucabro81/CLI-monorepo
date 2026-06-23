---
name: add-cli-command
description: Use when adding a new command or subcommand to any CLI crate in this monorepo (crates/jira, crates/bitbucket, future crates). Walks through scoping, REST API research, TDD implementation, and a verification loop until the command works end-to-end against a real account/site/workspace, following this repo's conventions. Crate-specific details (API docs URL, pagination style, scope/permission system, e2e setup) live in that crate's ADDENDUM.md, which this skill reads as it goes.
---

# Adding a command to a CLI crate

This skill drives the full lifecycle of adding one command (or subcommand) to
a crate in this monorepo. Follow the steps in order. Don't skip the research
or the verification loop — "compiles and unit tests pass" is not "done" for
these crates.

## 0. Identify the crate and load its addendum

Determine the target crate from the current working directory (e.g. cwd
`crates/bitbucket` -> crate `bitbucket`) or from what the user asked. If
ambiguous, ask.

Read, in order:

1. The workspace root `CLAUDE.md` (monorepo-wide rules: TDD, error handling,
   flag conventions, structure convention).
2. `crates/<crate>/CLAUDE.md` (module map, auth design, API design notes,
   implemented commands table).
3. `crates/<crate>/.claude/skills/add-<crate>-command/ADDENDUM.md` — the
   crate-specific facts this skill refers to throughout: API docs URL,
   pagination style, scope/permission system and where it's checked,
   destructive-command precedent, BACKLOG ID prefixes, live-test
   target (project/site/workspace), and whether e2e tests exist.

All three are short. If the addendum is missing or a crate has none yet,
note that and proceed with generic defaults, flagging gaps in the final
report.

## 1. Scope the command — ask, don't assume

Before writing anything, make sure these are nailed down. Use AskUserQuestion
for anything ambiguous — guessing wrong here means rework later. This initial
briefing is the most important step: the rest of this skill runs largely
unsupervised, so anything not pinned down here becomes an assumption baked
into the implementation.

Check `BACKLOG.md` (workspace root) for existing entries with the addendum's
ID prefixes — a planned edge case or design note may be directly relevant to
the command being added.

- **Command shape**: top-level (`Command` enum) or subcommand of an existing
  group (per the addendum's command-group list)? Exact name — long,
  descriptive, no abbreviations (per root CLAUDE.md).
- **Inputs**: required vs optional flags, all `#[arg(long)]`, no short
  aliases. Follow the addendum's precedent for typed-flags-vs-raw-body.
- **Output**: what JSON shape does it return? Full API response, or a small
  synthesized object (e.g. `{"deleted": true, ...}`)?
- **Destructive?** If it deletes/modifies state irreversibly, it needs an
  explicit `--confirm` flag — no interactive prompts, an LLM can't respond to
  those. The error message should include the exact retry command. See the
  addendum for this crate's existing precedent.
- **Auth/scope impact**: does this need a permission/scope not currently
  granted? Check this crate's `doctor` output against the endpoint's
  documented requirement (see step 2). If a new scope/permission is needed,
  flag this explicitly to the user — see the addendum for what human step
  (re-consent, OAuth consumer edit, etc.) that requires, and whether `doctor`
  itself needs a new check (usually not — see addendum).

## 2. Research the REST API endpoint

Don't guess endpoint paths or payload shapes. For each endpoint involved:

1. Check the official docs (URL in the addendum) for: HTTP method, path,
   required/optional params, request body shape, response shape, and the
   permission/scope required.
2. Check the response's pagination style against the addendum (e.g.
   cursor-based `nextPageToken` vs page-number `?page=N`) — follow the
   existing pattern for this crate, don't introduce a new one without reason.
3. Note any crate-specific encoding requirements (e.g. Atlassian Document
   Format for text fields — see addendum if relevant).
4. Note the exact path string(s)/path-builder function(s) — these go into
   `endpoints.rs`, not inlined in `client.rs`/`auth.rs`.

If the endpoint behavior is uncertain (e.g. exact field names, or whether a
field is required), it's faster to do a quick manual `curl`/`cargo run` probe
against the real test target (see addendum) than to keep re-reading docs. Ask
the user for a project/site/workspace identifier if one isn't already
established in the conversation.

## 3. Design — write it down before coding

Sketch (briefly, in your own response, not a separate doc):

- The `endpoints.rs` constant(s)/path-builder function(s) needed.
- The `client.rs` method signature (return type: raw `serde_json::Value`
  unless there's a strong reason for a typed struct, per existing pattern;
  reuse existing `get_json`/`post_json`-style helpers, adding a new one only
  if genuinely needed).
- Any new `ClientError`/`CliError`/`LoginError` variants — `thiserror`,
  message = problem + corrective action, no `unwrap`/`expect` outside tests.
- The `cli.rs` struct/variant, with `after_help` examples if the command has
  multiple meaningful flag combinations.
- The `commands/<group>.rs` handler function signature, and any pure helper
  functions (e.g. body builders, identifier splitters) that should be
  unit-tested in isolation from the network call. Check whether a helper
  needed here already exists in a shared location (e.g. `context.rs`) before
  writing a new one.

Then implement **incrementally, one logical unit at a time** (per root
CLAUDE.md): e.g. endpoints → client method → error variants → cli parsing →
handler → dispatch, each as its own edit with a one-line description of what
it does and why, so the pieces remain individually reviewable even though the
loop runs end-to-end without pausing for approval between them.

## 4. TDD — red, then green

Per root CLAUDE.md, tests must exist and fail before implementation:

1. **`cli_tests.rs`**: write parsing tests for the new command/flags first
   (e.g. `parses_<group>_<new>_with_no_optional_flags`,
   `parses_<group>_<new>_with_all_flags`). Run `cargo test -p <crate>` —
   confirm these fail to compile or fail because the variant doesn't exist
   yet.
2. If the command involves new client-side logic beyond a passthrough HTTP
   call (e.g. body construction, response reshaping, identifier splitting) —
   write unit tests for that logic first too.
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
6. `cargo test -p <crate>` — all green. `cargo clippy -p <crate>
   --all-targets` — zero warnings (fix `pedantic` lints too, don't `#[allow]`
   them away unless there's a real reason).

## 5. Manual smoke test

```sh
cargo run -p <crate> -- <command> --help          # accurate, complete help text?
cargo run -p <crate> -- <command> ...             # against the real test target
```

Iterate here against the live API until the output looks right — this is
where wrong endpoint assumptions from step 2 usually surface (e.g. an
unexpected 4xx because of an account/workspace-level constraint, a field name
that differs from the docs).

If the command is destructive or creates persistent state, tell the user what
was created/modified in the real target as part of the final report (step 8)
— don't silently leave test artifacts without mentioning them.

## 6. Extended verification (e2e, if this crate has it)

Check the addendum: some crates have an automated e2e suite, others rely on
step 5's manual live verification only. If this crate has e2e tests, add one
following the addendum's conventions (cleanup helpers, naming prefixes,
self-contained scoping, thread-safety constraints).

If this is the **first** e2e test for the crate, the `src/tests/e2e_tests.rs`
file itself doesn't exist yet and isn't created by the scaffold script
(`new-cli-crate`'s `new-crate.sh` deliberately skips it — see its header
comment). Create it now, mirroring an existing crate's e2e_tests.rs (e.g.
`crates/jira/src/tests/e2e_tests.rs`): a module doc-comment stating
prerequisites (credentials, any required env var) and the run command
(`cargo test -p <crate> -- --ignored`), every test annotated
`#[ignore = "e2e: requires ..."]`, and a `setup()` helper that loads
credentials and builds the authenticated client. Wire it into `main.rs`:

```rust
#[cfg(test)]
#[path = "tests/e2e_tests.rs"]
mod e2e_tests;
```

Not every command needs e2e coverage — see the addendum for this crate's
scope (e.g. read-only-only, or some commands excluded because they create
real visible/destructive state and aren't safe to run unattended).

## 7. Verification loop — run until it actually works

Repeat until everything below is green, fixing root causes (not loosening
assertions or adding `--allow` to silence problems):

```sh
cargo test -p <crate>
cargo clippy -p <crate> --all-targets
```

Plus this crate's e2e command, if applicable (see addendum).

If a test fails because an assumption from step 2 was wrong (wrong endpoint,
wrong permission/scope, wrong response field name), go back to step 2,
correct it, and re-run the full loop — don't patch around it locally.

## 8. Docs and commit

- `crates/<crate>/README.md`: add a `### <crate> <command>` section with
  usage, flags, required scope/permission, and at least one example,
  following the style of existing entries.
- `crates/<crate>/CLAUDE.md`: update the "Status" line, module map
  annotations, and the "Implemented commands"/"Planned commands" tables.
  Update "API design notes" if this introduced a new pattern (new pagination
  style, new HTTP method helper, etc.).
- `BACKLOG.md` (workspace root): note any edge cases discovered but not
  handled, with an ID following the addendum's prefix convention.
- Commit in small atomic units per root CLAUDE.md (e.g. one commit for
  client+endpoints+cli+handler+unit tests+docs if it's genuinely one logical
  unit, separate commits for unrelated refactors or backlog notes). Each
  commit message ends with `Co-Authored-By: Claude Sonnet 4.6`.
- **Every commit message must use `<crate>` as the conventional-commit scope**
  (`feat(<crate>): ...`, `fix(<crate>): ...`, `docs(<crate>): ...`) — this is
  not just style. `release-plz` (root CLAUDE.md "CI/CD" section) reads the
  scope to attribute the commit to this crate and compute its version bump
  independently from other crates. An unscoped commit, a wrong scope, or one
  commit spanning multiple crates breaks that — split into separate scoped
  commits if a change genuinely touches more than one crate.
- Push the branch and **open a PR against `main`** — don't push commits
  directly to `main`. The PR's CI run (`.github/workflows/ci.yml`) is the
  build/test/clippy gate; merging it is what eventually triggers
  `release-plz` to draft a release PR for this crate (see root CLAUDE.md).

## 9. Final report

This skill is meant to run largely unsupervised end-to-end — possibly invoked
by an agent that is itself a user of this CLI, not a human watching every
step. Compensate for that with a **detailed final report** to the user,
covering:

- What was added (command, flags, endpoint(s), required scope/permission,
  files touched, commits made).
- Every assumption made during step 1 that wasn't explicitly confirmed by the
  user (there shouldn't be many, but name them if they exist).
- Any persistent state created/modified in the live target during step 5
  (e.g. a test repository or issue) that the user may want to clean up.
- **Specific points the user should double-check**, called out clearly —
  e.g. "the scope/permission required couldn't be verified against a real
  4xx because the test account already has broad permissions", "this
  endpoint's pagination behavior was inferred from docs, not observed live
  with more than one page of results". Don't bury these in a wall of text —
  a short bulleted "needs human review" section at the end of the report.
- Final state of the verification loop (test/clippy/e2e/live results) and
  any known-skipped checks with a reason.
