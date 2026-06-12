# `jira` crate specifics for add-cli-command

Read alongside `.claude/skills/add-cli-command/SKILL.md` (workspace root) and
`crates/jira/CLAUDE.md` (already covers module map, OAuth/auth design, API
design notes, command table, BACKLOG prefixes — don't repeat that here, only
what's missing for the skill's process).

Section headings below match `SKILL.md`'s step numbers — only steps where
this crate deviates from or adds to the generic skill are covered here.
Steps not listed (3-5, 7-9) follow `SKILL.md` as-is.

## Step 1 — Scope

- **Permission check**: `jira doctor`'s `permissions` check
  (`commands/doctor.rs`, `PERMISSION_KEYS`) reports a fixed map of booleans
  for permissions the CLI relies on. If the new endpoint needs a permission
  not in that map, add the key there.
- A new OAuth scope requires re-running the `--user` consent flow (`jira
  init` / `jira auth login --user`) — a one-time human step.

## Step 2 — API research

Docs: `https://developer.atlassian.com/cloud/jira/platform/rest/v3/`
(use WebFetch/WebSearch).

## Step 6 — e2e tests

`crates/jira/src/e2e_tests.rs`, each test `#[ignore = "e2e: requires credentials and JIRA_E2E_PROJECT"]`:

- **Isolation**: every issue created by a test gets the `[jira-cli-e2e]`
  summary prefix (`e2e_summary()` helper); `IssueGuard` deletes it on drop
  (even on panic).
- **Self-contained**: create exactly the data the test needs and scope JQL
  queries to specific issue keys, rather than relying on project-wide state —
  see `e2e_search_pagination` for the pattern.
- **Concurrency**: `--test-threads=1` is a hard requirement (shared site state
  across project-wide JQL queries) — don't "fix" flakiness by parallelizing.
- **Sync with other checks**: if the new command changes `doctor`'s output
  shape (e.g. a new permission key), update `e2e_smoke_doctor`'s assertions
  too.
- **Running**:
  ```sh
  JIRA_E2E_PROJECT=KAN cargo test -p jira -- --ignored --test-threads=1
  # recovery
  JIRA_E2E_PROJECT=KAN cargo test -p jira e2e_cleanup -- --ignored
  ```
- **Extending**: add a new `#[ignore]` test following the `IssueGuard`
  pattern, or extend an existing lifecycle test if the new command fits
  there.
