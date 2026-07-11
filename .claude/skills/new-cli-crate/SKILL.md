---
name: new-cli-crate
description: Use when adding a brand-new CLI crate to this monorepo for a service that doesn't have one yet (e.g. "let's add a GitHub CLI"). Scaffolds the crate skeleton, drives the auth-design discussion, proposes a minimal command pool, writes the crate's CLAUDE.md/README/ADDENDUM, then bootstraps init/doctor/auth and the first commands via add-cli-command.
---

# Creating a new CLI crate

This skill takes a service name (e.g. "GitHub", "Slack") from zero to a
working crate with `init`/`doctor`/`auth`/`whoami` and a small set of agreed
core commands, following this repo's conventions. It hands off to
`add-cli-command` for the actual per-command implementation loop — this
skill's job is everything that has to happen *before* that loop can start.

Read the workspace root `CLAUDE.md` first if you haven't already (structure
convention, error handling, flag conventions).

## 0. Scaffold

```sh
.claude/skills/new-cli-crate/scripts/new-crate.sh <crate-name> "<short service description>"
```

This creates `crates/<crate-name>/` with `Cargo.toml` (already depending on
the shared `crates/cli-fields` crate for `--select` support — see root
`CLAUDE.md`'s "Shared library: crates/cli-fields"), the directory layout from
root `CLAUDE.md`, and placeholder `TODO` stubs for everything
service-specific. It also adds the crate to the workspace `members` list.
**It does not compile yet** — that's expected until step 4.

If the crate name is ambiguous (e.g. the service has a generic name, or
there's already a similarly-named crate), confirm with the user before
running this.

## 1. Auth design — research and decide before anything else

This is the step that most determines the shape of everything downstream, and
the one place this skill should slow down and discuss with the user rather
than barrel ahead.

1. **Research** how the service's API handles authentication: OAuth grant
   types available, whether `client_credentials` (no human interaction) is
   viable or whether a 3LO/PKCE-style human consent step is unavoidable,
   token lifetime/refresh behavior, scope model, and any multi-tenancy
   concept analogous to jira's `cloud_id` or bitbucket's workspace slug.
2. **Check for reuse first.** If this service's OAuth shape resembles an
   existing crate (Atlassian-family, or otherwise), read that crate's
   `auth.rs` and `CLAUDE.md` "Auth design" section, and check
   [[BACKLOG.md]] for `LIB-1` (shared Atlassian-auth library, currently
   deferred pending a third crate). If this crate would be that third
   instance, raise it explicitly with the user — this may be the point where
   `LIB-1` stops being premature.
3. **Decide and confirm with the user** (AskUserQuestion for anything
   ambiguous): grant type, scopes needed for the baseline commands (step 2),
   config file layout (`app.json` / `credentials.json` under
   `$XDG_CONFIG_HOME/<crate>-cli/`, matching existing crates unless there's a
   reason to deviate), and whether a one-time human bootstrap step
   (`init --user` equivalent) is required.
4. Write this decision down as a draft "Auth design" section for
   `crates/<crate>/CLAUDE.md` (you'll place it properly in step 3) — don't
   leave it only in conversation, the bootstrap step (step 4) implements
   against it.

## 2. Command pool — propose, don't assume

Per root `CLAUDE.md`'s incremental approach: "start with the smallest useful
command set, add new commands only when a concrete need arises." Propose:

- **Baseline (always)**: `init`, `doctor`, `auth login`, `auth whoami` —
  these exist in every crate so far and are the prerequisite for verifying
  anything else end-to-end.
- **Core (2-4 commands)**: the smallest set that makes the CLI useful for a
  realistic LLM-agent task with this service — e.g. for jira this was
  `issue get`/`create`/`search`. Base this on what the service is *for*, not
  an exhaustive API survey.

Present this pool to the user with brief justification for each command and
get their reaction/adjustments before proceeding — this is a second
checkpoint, separate from the auth discussion, because the user may want a
different starting slice than what seems obvious from the API docs.

## 3. Write the docs

With auth design (step 1) and command pool (step 2) agreed:

- `crates/<crate>/CLAUDE.md` — module map (mirror an existing crate's, e.g.
  bitbucket's, adjusted for this service), "Auth design" section from step
  1, "Implemented commands" table (empty/TODO for now — filled in as step 4/5
  land each command), "Planned commands" table from step 2's core list,
  config layout, API design notes (pagination style if known).
- `crates/<crate>/README.md` — skeleton with the same sections as
  jira/bitbucket's READMEs (Table of contents, Setup, How the OAuth flow
  works, Usage, Testing, Error design), Setup and OAuth sections filled in
  from step 1, Usage left as TODO per command.
- `crates/<crate>/.claude/skills/add-<crate>-command/ADDENDUM.md` — following
  the structure of `crates/jira/.claude/skills/add-jira-command/ADDENDUM.md`
  or `crates/bitbucket/.claude/skills/add-bitbucket-command/ADDENDUM.md`:
  permission/scope check location, API docs URL, e2e conventions (or "no e2e
  yet" if deferred), BACKLOG ID prefix for this crate.
- Root `README.md` — add the crate's row to the CLI registry table (per root
  `CLAUDE.md`'s Development approach).

Commit this as its own small commit ("docs: scaffold <crate> crate docs and
ADDENDUM") — it's a coherent unit separate from the code that follows. This
commit has no crate-specific feature, so it's fine without a `(<crate>)`
scope; every commit from step 4 onward (real code) must use `<crate>` as the
conventional-commit scope, per `add-cli-command`'s commit-and-PR step — this
is what lets the release pipeline (root CLAUDE.md "CI/CD" section) attribute
commits to this crate and compute its version bumps.

Unlike release-plz (which this pipeline replaced), the new git-cliff +
cargo-release pipeline does **not** auto-discover workspace crates — the
crate list is a hardcoded matrix in `.github/workflows/release-pr.yml` and
`release-tag.yml`. Wiring a new crate into the release pipeline (do this
once the crate has its first real command, not at scaffold time) requires:
add `<crate>` to both workflows' `matrix.crate` list; add `publish = false`
and a `[package.metadata.release]` `pre-release-hook` to its `Cargo.toml`
(copy the block from `crates/jira/Cargo.toml`, only the crate name differs —
it's parameterized via `$CRATE_NAME`); create an empty `crates/<crate>/CHANGELOG.md`
with the standard Keep-a-Changelog header (copy from any existing crate) so
git-cliff's `--prepend` has something to prepend to.

## 4. Bootstrap — init, doctor, auth

Invoke `add-cli-command` for `auth login` (and `auth whoami`) first, then
`doctor`, then `init` — in that order, since `doctor` calls into auth checks
and `init` calls `doctor` as its final verification (see existing crates'
`commands/init.rs`). This is the step where the placeholder files from step 0
get real content and the crate starts compiling.

Run `cargo build -p <crate>` and `cargo clippy -p <crate> --all-targets`
after this step — the crate must build clean before moving on.

## 5. Core commands

Loop `add-cli-command` over the commands agreed in step 2, one at a time,
same as adding any command to an existing crate.

## 6. Final report

Same shape as `add-cli-command`'s step 9: what was created (crate, files,
commits), the auth design decided and why, the command pool and rationale,
any `BACKLOG.md` entries added (e.g. if `LIB-1` was acted on or deferred
again), and a "needs human review" section — most importantly **any
human-side setup required** (creating an OAuth app/consumer, granting
scopes/permissions) before `doctor`/`init` can pass for real.

Do all of the above on a branch and open a PR against `main` (per
`add-cli-command`'s commit-and-PR step) rather than pushing straight to
`main` — include the PR link in the final report. Before merging, make sure
the release-pipeline wiring from step 3 (matrix entries, `Cargo.toml`
`publish = false` + `pre-release-hook`, initial `CHANGELOG.md`) is included,
or the new crate's `feat`/`fix` commits will silently never trigger a
release PR.
