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

None. This crate has no automated e2e suite — step 5's manual live
verification against the real workspace is the only check beyond unit tests.
