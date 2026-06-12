# `bitbucket` crate specifics for add-cli-command

Read alongside `.claude/skills/add-cli-command/SKILL.md` (workspace root) and
`crates/bitbucket/CLAUDE.md` (already covers module map, auth/scope design,
API design notes, command tables, BACKLOG prefixes — don't repeat that here,
only what's missing for the skill's process).

## Step 1 — Scope

- A new OAuth scope requires editing the workspace OAuth consumer (Settings →
  OAuth consumers → edit) — a one-time human step. Per `DOCTOR-1`, do **not**
  add a new key to `doctor`'s permissions check for this; it reports
  `granted_scopes` as-is.

## Step 2 — API research

Docs: `https://developer.atlassian.com/cloud/bitbucket/rest/`
(use WebFetch/WebSearch).

## Live test target

Workspace `lucabrognaracode`, repos `lucabrognaracode/repo-test` and
`lucabrognaracode/cli-test-repo`. Ask the user if a different
workspace/repo slug is needed.

## Step 6 — e2e tests

`crates/bitbucket/src/e2e_tests.rs`, wired into `main.rs` behind
`#[cfg(test)]`, each test `#[ignore = "e2e: requires credentials, git, and a writable Bitbucket workspace"]`:

- **Isolation**: every repo created by a test gets the
  `cli-bitbucket-e2e-<label>-<timestamp>` slug (`e2e_repo_slug()` helper);
  `RepoGuard` deletes it on drop (even on panic).
- **Self-contained**: `e2e_pr_lifecycle` creates its own repo, pushes its own
  branches via `git` over HTTPS (`x-token-auth` + OAuth access token), and
  opens its own pull requests — no reliance on pre-existing workspace state
  (e.g. `repo-test`/`cli-test-repo`).
- **Concurrency**: `--test-threads=1` — each test owns its own repo so
  parallel runs aren't strictly unsafe, but keep sequential to avoid
  hammering the API and to match other crates' convention.
- **Sync with other checks**: n/a currently — `doctor`'s `permissions` check
  reports raw `granted_scopes` with no fixed permission map to keep in sync
  (see `DOCTOR-1`).
- **Running**:
  ```sh
  cargo test -p bitbucket -- --ignored --test-threads=1
  # recovery
  cargo test -p bitbucket e2e_cleanup -- --ignored
  ```
- **Extending**: extend `e2e_pr_lifecycle` if the new command fits the pr
  lifecycle, or add a new `#[ignore]` test following the `RepoGuard` pattern
  otherwise. Step 5's manual live verification remains the primary check for
  commands outside this lifecycle.
