# `atlassian-admin` crate specifics for add-cli-command

Read alongside `.claude/skills/add-cli-command/SKILL.md` (workspace root) and
`crates/atlassian-admin/CLAUDE.md` (already covers module map, auth design,
API design notes, command tables — don't repeat that here, only what's
missing for the skill's process).

Section headings below match `SKILL.md`'s step numbers — only steps where
this crate deviates from or adds to the generic skill are covered here.
Steps not listed follow `SKILL.md` as-is.

## Step 1 — Scope

- A new scope requires editing the Organization API key at admin.atlassian.com
  (Organization settings → API keys) — keys can't have scopes added in place;
  check whether the existing key's scopes (`read:accounts:admin`,
  `read:directories:admin`) already cover the new command before asking the
  user to create a new key with a wider scope set. Scope catalog:
  `https://developer.atlassian.com/cloud/admin/scopes/`.
- There is no `doctor` permissions check to keep in sync (unlike
  jira/bitbucket) — `doctor` only checks `app_config` and a live `api` call,
  see CLAUDE.md.

## Step 2 — API research

Docs: `https://developer.atlassian.com/cloud/admin/organization/rest/`
(use WebFetch/WebSearch — this documentation set has been unreliable to
fetch directly in practice; cross-check with a live curl call against the
real API using the configured key wherever possible instead of trusting a
single fetch).

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
