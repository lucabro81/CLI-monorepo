# `jira` crate specifics for add-cli-command

Read alongside `.claude/skills/add-cli-command/SKILL.md` (workspace root).
This file has no frontmatter — it is not an invocable skill on its own, just
reference material the root skill reads.

## Step 1 — Scope

- **Command groups**: `IssueCommand`, `AuthCommand`, `CommentCommand`.
- **BACKLOG ID prefixes**: `FIELDS-*`, `CREATE-*` (and others — grep
  `BACKLOG.md` for `(jira)`).
- **Destructive-command precedent**: `issue delete` — explicit `--confirm`
  flag, no interactive prompt.
- **Auth/scope system**: OAuth scopes are
  `read:jira-work read:jira-user write:jira-work offline_access` (see
  `crates/jira/CLAUDE.md` → OAuth / auth design). A new scope requires
  re-running the `--user` consent flow (`jira init` / `jira auth login
  --user`) — a one-time human step.
- **Permission check**: `jira doctor`'s `permissions` check
  (`commands/doctor.rs`, `PERMISSION_KEYS`) reports a fixed map of booleans
  for permissions the CLI relies on. If the new endpoint needs a permission
  not in that map, add the key there.

## Step 2 — API research

- Docs: `https://developer.atlassian.com/cloud/jira/platform/rest/v3/`
  (use WebFetch/WebSearch).
- **ADF**: if the endpoint takes a text field (description, comment), it
  needs Atlassian Document Format — reuse the inline ADF construction
  pattern from `client.rs` (`add_comment`, `create_issue`).
- **Pagination**: cursor-based `nextPageToken` (see `search_issues`) — not
  offset/page-number based.

## Live test target

Real Jira project `JIRA_E2E_PROJECT` (e.g. `KAN`).

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

## Step 7 — verification loop command

```sh
cargo test -p jira
cargo clippy -p jira
JIRA_E2E_PROJECT=<your-test-project> cargo test -p jira -- --ignored --test-threads=1
```

`--test-threads=1` for e2e is a hard requirement (shared site state across
project-wide JQL queries) — don't try to "fix" flakiness by parallelizing.
