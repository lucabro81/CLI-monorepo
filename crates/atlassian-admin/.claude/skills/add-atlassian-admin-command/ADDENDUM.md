# `atlassian-admin` crate specifics for add-cli-command

Read alongside `.claude/skills/add-cli-command/SKILL.md` (workspace root) and
`crates/atlassian-admin/CLAUDE.md` (already covers module map, auth design,
API design notes, command tables — don't repeat that here, only what's
missing for the skill's process).

Section headings below match `SKILL.md`'s step numbers — only steps where
this crate deviates from or adds to the generic skill are covered here.
Steps not listed follow `SKILL.md` as-is.

## Step 1 — Scope

- The configured key is unscoped ("without scopes", full access) — see
  CLAUDE.md's "Auth design" for why (`user get`'s required scope,
  `manage:org`, isn't in Atlassian's public scope catalog and can't be
  selected on a scoped key at all). A new command almost certainly already
  has access; there's no scope catalog to check against in practice for this
  crate the way there is for jira/bitbucket. If a future command somehow
  needs *more* than full org-admin access grants (unlikely), that's a
  different problem than scope selection.
- There is no `doctor` permissions check to keep in sync (unlike
  jira/bitbucket) — `doctor` only checks `app_config` and a live `api` call,
  see CLAUDE.md.

## Step 2 — API research

Docs: `https://developer.atlassian.com/cloud/admin/organization/rest/`
(use WebFetch/WebSearch — this documentation set has been **confirmed
unreliable** in practice, not just slow: `user get`'s original path and
scope were both wrong despite looking confirmed by WebFetch during design,
and only surfaced once tested against a real organization — see CLAUDE.md's
"Corrections found via live testing"). **Do not write an endpoint path or
scope name into code/docs from a WebFetch result alone.** Cross-check with a
live request first — a deliberately-unscoped or wrong-scope key still
distinguishes a `403` (route exists, scope wrong — check the response body's
listed acceptable scopes) from a `404` (route doesn't exist, path is wrong)
without needing full permissions yet.

## Live test target

No dedicated test organization — test against the real organization backing
the configured `org_id`. Since this API is read-only admin/directory data
(no create/delete commands planned), there's no throwaway-resource pattern
to follow like bitbucket's `RepoGuard`. Ask the user before adding any
command that would mutate organization state (none planned currently).

## Step 6 — e2e tests

No automated e2e suite yet (see CLAUDE.md's "Status"). If a second command
is added, revisit whether one is warranted — likely a thin `#[ignore]` live
check per command (verify a known real `account_id` resolves), not a
multi-step lifecycle test like bitbucket's pr lifecycle, since this API's
surface is read-only lookups, not create/mutate/delete sequences.

## BACKLOG.md prefix

Use `ADMIN-` for new entries as commands are implemented.
