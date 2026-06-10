---
name: add-jira-command
description: Use when adding a new command or subcommand to the jira CLI crate (crates/jira). Walks through scoping, Jira REST API research, TDD implementation, e2e testing, and a fix-verify loop until the command works end-to-end against a real Jira site, following this repo's conventions.
---

# Adding a command to the `jira` CLI

This skill drives the full lifecycle of adding one command (or subcommand) to
`crates/jira`. Follow the steps in order. Don't skip the research or the e2e
loop — "compiles and unit tests pass" is not "done" for this crate.

Read the workspace root `CLAUDE.md` (monorepo-wide rules:
TDD, error handling, flag conventions) and `crates/jira/CLAUDE.md` (module map,
OAuth design, API design notes, implemented commands table) before starting —
both are short and define conventions this skill assumes.

## 1. Scope the command — ask, don't assume

Before writing anything, make sure these are nailed down. Use AskUserQuestion
for anything ambiguous — guessing wrong here means rework later:

- **Command shape**: top-level (`Command` enum) or subcommand of an existing
  group (`IssueCommand`, `AuthCommand`, `CommentCommand`)? Exact name —
  long, descriptive, no abbreviations (per root CLAUDE.md).
- **Inputs**: required vs optional flags, all `#[arg(long)]`, no short aliases.
- **Output**: what JSON shape does it return? Full Jira response, or a small
  synthesized object (like `{"deleted": true, "key": "KAN-5"}`)?
- **Destructive?** If it deletes/modifies state irreversibly, it needs an
  explicit `--confirm` flag (see `issue delete` for the pattern) — no
  interactive prompts, an LLM can't respond to those.
- **Auth/scope impact**: does this need a Jira permission or OAuth scope not
  currently granted? Current scopes are
  `read:jira-work read:jira-user write:jira-work offline_access`
  (see `crates/jira/CLAUDE.md` → OAuth / auth design). If a new scope is
  needed, flag this explicitly to the user — it requires re-running the
  `--user` consent flow (`jira init` / `jira auth login --user`), which is a
  one-time human step they need to be aware of.

## 2. Research the Jira REST API endpoint

Don't guess endpoint paths or payload shapes. For each endpoint involved:

1. Check the official docs at
   `https://developer.atlassian.com/cloud/jira/platform/rest/v3/` (use
   WebFetch/WebSearch) for: HTTP method, path, required/optional params,
   request body shape, response shape, and the **permission** required
   (this maps to a key checked by `jira doctor`'s `permissions` check —
   see `commands/doctor.rs` `PERMISSION_KEYS`; add the new key there if
   relevant).
2. Confirm whether the endpoint needs Atlassian Document Format (ADF) for
   any text field (descriptions, comments) — if so, reuse the inline ADF
   construction pattern from `client.rs` (`add_comment`, `create_issue`).
3. Check whether the response is paginated (cursor-based `nextPageToken`,
   like `search_issues`) — if so, follow that pattern, not offset-based
   pagination.
4. Note the exact path string(s) — these go into `endpoints.rs`, not
   inlined in `client.rs`/`auth.rs`.

If the endpoint behavior is uncertain (e.g. exact permission key, or whether
a field is required), it's faster to do a quick manual `curl`/`cargo run`
probe against the real `KAN` test project than to keep re-reading docs.

## 3. Design — write it down before coding

Sketch (briefly, in your own response, not a separate doc):

- The `endpoints.rs` constant(s)/path-builder function(s) needed.
- The `client.rs` method signature (return type: raw `serde_json::Value`
  unless there's a strong reason for a typed struct, per existing pattern).
- Any new `ClientError`/`CliError`/`LoginError` variants — `thiserror`,
  message = problem + corrective action, no `unwrap`/`expect` outside tests.
- The `cli.rs` struct/variant, with `after_help` examples if the command has
  multiple meaningful flag combinations.
- The `commands/<group>.rs` handler function signature.

## 4. TDD — red, then green

Per root CLAUDE.md, tests must exist and fail before implementation:

1. **`cli_tests.rs`**: write parsing tests for the new command/flags first
   (e.g. `parses_issue_<new>_command`, plus a test for any required-flag
   validation). Run `cargo test -p jira` — confirm these fail to compile or
   fail because the variant doesn't exist yet.
2. If the command involves new client-side logic beyond a passthrough HTTP
   call (e.g. new `fields.rs` projection behavior, new ADF construction,
   response reshaping) — write unit tests for that logic first too.
3. Implement the minimum code to make these pass: `endpoints.rs` →
   `client.rs` → `error.rs` (if needed) → `cli.rs` → `commands/<group>.rs` →
   `main.rs` dispatch.
4. `cargo test -p jira` — all green. `cargo clippy -p jira` — zero warnings
   (fix `pedantic` lints too, don't `#[allow]` them away unless there's a
   real reason).

## 5. Manual smoke test

```sh
cargo run -p jira -- <command> --help          # accurate, complete help text?
cargo run -p jira -- <command> ...              # against the real KAN project
```

Iterate here against the live API until the output looks right — this is
where wrong endpoint assumptions from step 2 usually surface.

## 6. E2e test

Add a test in `e2e_tests.rs`, `#[ignore = "e2e: requires credentials and JIRA_E2E_PROJECT"]`:

- Any issue created by the test gets the `[jira-cli-e2e]` summary prefix
  (`e2e_summary()` helper) and an `IssueGuard` for cleanup-on-drop.
- Prefer **self-contained** tests (create exactly the data the test needs,
  scope JQL queries to specific issue keys) over relying on project-wide
  state — see `e2e_search_pagination` for the pattern.
- If the new command changes `doctor`'s output shape (e.g. a new permission
  key), update `e2e_smoke_doctor`'s assertions too.

## 7. Verification loop — run until it actually works

Repeat until everything below is green, fixing root causes (not loosening
assertions or adding `--allow` to silence problems):

```sh
cargo test -p jira
cargo clippy -p jira
JIRA_E2E_PROJECT=KAN cargo test -p jira -- --ignored --test-threads=1
```

`--test-threads=1` for e2e is a hard requirement in this crate (shared site
state across project-wide JQL queries) — don't try to "fix" flakiness by
parallelizing.

If a test fails because an assumption from step 2 was wrong (wrong endpoint,
wrong permission key, wrong response field name), go back to step 2, correct
it, and re-run the full loop — don't patch around it locally.

## 8. Docs and commit

- `crates/jira/README.md`: add a `### jira <command>` section with usage,
  flags, and at least one example, following the style of existing entries.
- `crates/jira/CLAUDE.md`: update the "Implemented commands" table, and the
  module map / API design notes if this introduced a new pattern (new ADF
  usage, new pagination style, new permission key, etc.).
- `BACKLOG.md` (workspace root): note any edge cases discovered but not
  handled, with an ID following the existing `<AREA>-<N>` convention.
- Commit in small atomic units per root CLAUDE.md (e.g. one commit for
  client+endpoints+cli+handler+unit tests, one for e2e test, one for docs —
  or combine if it's genuinely one logical unit). Each commit message ends
  with `Co-Authored-By: Claude Sonnet 4.6`.
