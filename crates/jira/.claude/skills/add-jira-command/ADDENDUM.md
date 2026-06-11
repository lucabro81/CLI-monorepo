# `jira` crate specifics for add-cli-command

Read alongside `.claude/skills/add-cli-command/SKILL.md` (workspace root) and
`crates/jira/CLAUDE.md` (already covers module map, OAuth/auth design, API
design notes, command table, BACKLOG prefixes — don't repeat that here, only
what's missing for the skill's process).

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

`crates/jira/src/e2e_tests.rs`, each test
`#[ignore = "e2e: requires credentials and JIRA_E2E_PROJECT"]`:

- Any issue created by a test gets the `[jira-cli-e2e]` summary prefix
  (`e2e_summary()` helper) and an `IssueGuard` for cleanup-on-drop.
- Prefer **self-contained** tests (create exactly the data the test needs,
  scope JQL queries to specific issue keys) over relying on project-wide
  state — see `e2e_search_pagination` for the pattern.
- If the new command changes `doctor`'s output shape (e.g. a new permission
  key), update `e2e_smoke_doctor`'s assertions too.
- `--test-threads=1` is a hard requirement (shared site state across
  project-wide JQL queries) — don't try to "fix" flakiness by parallelizing.
