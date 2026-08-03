# `confluence` crate specifics for add-cli-command

Read alongside `.claude/skills/add-cli-command/SKILL.md` (workspace root) and
`crates/confluence/CLAUDE.md` (already covers module map, auth/scope design,
API design notes, command tables, known gaps — don't repeat that here, only
what's missing for the skill's process).

Section headings below match `SKILL.md`'s step numbers — only steps where
this crate deviates from or adds to the generic skill are covered here.
Steps not listed follow `SKILL.md` as-is.

## Step 1 — Scope

- A new OAuth scope requires editing the app/Service Account credential in
  developer.atlassian.com or admin.atlassian.com (whichever Setup option was
  used — see this crate's README) — a one-time human step, same as `jira`.
- Check whether the new command has a v1 (`/wiki/rest/api/...`) or v2
  (`/wiki/api/v2/...`) endpoint first — prefer v2 whenever one exists (see
  CLAUDE.md's "API design notes"). Confluence's scope model is split the
  same way: v2 endpoints only accept granular scopes
  (`<verb>:<resource>:confluence`), v1 endpoints generally use classic
  scopes (`<verb>:confluence-<resource>`). Don't assume a scope already in
  `SCOPES` covers a new endpoint just because it's "close" — verify against
  [developer.atlassian.com/cloud/confluence/scopes-for-oauth-2-3LO-and-forge-apps](https://developer.atlassian.com/cloud/confluence/scopes-for-oauth-2-3LO-and-forge-apps/).

## Step 2 — API research

Docs: `https://developer.atlassian.com/cloud/confluence/rest/v2/intro/` (v2)
and `https://developer.atlassian.com/cloud/confluence/rest/v1/intro/` (v1,
for anything v2 doesn't cover yet). Use WebFetch/WebSearch — this crate's
existing endpoint choices (see CLAUDE.md) were researched this way, not from
memory, and several details (template creation having no direct API, the
exact classic-vs-granular scope split) only came out of that research, not
from assumptions.

## Live test target

**None chosen yet** — this crate has no e2e tests and has not been
authenticated against a real Confluence site (see CLAUDE.md's "Known gaps").
Ask the user which site/space to use for live verification and e2e tests
before implementing a command that needs one; don't assume `jira`'s test
project (`MER`) has a corresponding Confluence space — check first.

## Step 5 — Manual live verification

Since there is no live-verified account yet, the first command implemented
after this crate's initial batch (`page get/create/update/search`,
`space list`) is also the first opportunity to actually confirm the scope
table in CLAUDE.md's "OAuth / auth design" is correct — expect to correct
scope names there based on real 403 responses, same as `atlassian-admin`'s
CLAUDE.md documents doing for its own scopes ("Corrections found via live
testing").

## Step 6 — e2e tests

Not set up yet. When adding the first one, follow `jira`'s
`tests/e2e_tests.rs` pattern: a `PageGuard` (RAII, mirroring jira's
`IssueGuard`) that deletes/trashes any page created during a test, an
`e2e_cleanup` recovery test that sweeps up orphans by a name prefix (e.g.
`[confluence-cli-e2e]` in the page title), and `--test-threads=1` if any
test relies on search results being stable across concurrent runs. Wire
`mod e2e_tests` into `main.rs` behind `#[cfg(test)]`, same as every other
crate.
