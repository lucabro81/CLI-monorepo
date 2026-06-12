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

`src/e2e_tests.rs`, wired into `main.rs` behind `#[cfg(test)]`. One ignored
lifecycle test (`e2e_pr_lifecycle`) creates a throwaway repo
(`cli-bitbucket-e2e-pr-<timestamp>`), pushes branches via `git` over HTTPS
using the OAuth access token (`x-token-auth`), and exercises pr
create/get/list/comment/approve/unapprove/merge/decline + branch list.
`RepoGuard` deletes the repo on drop. `e2e_cleanup` is the recovery test —
deletes any orphaned `cli-bitbucket-e2e-*` repos.

```sh
cargo test -p bitbucket -- --ignored --test-threads=1
```

When adding a new command, extend `e2e_pr_lifecycle` (or add a new ignored
test following the same `RepoGuard` pattern) if it fits the PR lifecycle;
otherwise step 5's manual live verification remains the primary check.
