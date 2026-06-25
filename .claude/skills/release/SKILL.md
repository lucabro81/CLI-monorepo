---
name: release
description: Use when the user asks to release, ship, publish, or cut a version for one crate in this monorepo (e.g. "release jira", "fai una release di google-chat", "ship bitbucket 0.3.0", "tag a new google-chat version"). Drives the release-plz-based release for exactly one crate end-to-end — checks whether a normal release-plz PR will appear or whether the publish=false registry-check workaround is needed, gets the version bump confirmed, opens/merges the right PR, watches the resulting GitHub Actions runs, and does a final check that the GitHub Release and its platform binaries actually exist. Use this instead of manually watching `gh run list` after a merge, and instead of guessing whether release-plz "did its thing."
---

# Releasing a crate in this monorepo

This skill takes one crate from "recent commits are merged to `main`" to "a
GitHub Release exists with its binaries attached", handling a known quirk in
this repo's release-plz setup along the way. It releases **one crate per
invocation** — if the user wants several, run it again for the next one
rather than trying to interleave the monitoring.

Read root `CLAUDE.md`'s "CI/CD" section first if you haven't already — it
documents the three workflows this skill drives:
[`.github/workflows/release-plz.yml`](../../../.github/workflows/release-plz.yml)
(jobs `Release-plz PR` and `Release-plz release`, both run on every push to
`main`), and
[`.github/workflows/release.yml`](../../../.github/workflows/release.yml)
(triggered by the `<crate>-v<version>` tag, builds the binary on three
runners and attaches it to the Release).

## Why this needs a skill, not just "wait for release-plz"

Every crate in this workspace has `publish = false` (these are internal
CLIs, never published to crates.io). release-plz's `release-pr` job decides
whether to bump a crate's version by comparing the local `Cargo.toml`
version against the version on crates.io — for a crate that's never been
published, that lookup returns "Package not found", and release-plz silently
treats the crate as already up to date instead of computing a bump from the
conventional commits since its last tag. **This only breaks a crate's
*second and later* releases** — the very first release of a brand-new crate
works automatically, because there's no prior tag to compare against and
release-plz handles that case differently.

Confirmed twice in this repo so far (commits `29e5ada` for jira, `bc99eb2`
for google-chat): a real `feat`/`fix` commit lands on `main`, no "chore:
release" PR appears, and `gh run view <run-id> --log` for the `Release-plz
PR` job shows `WARN Package '<crate>@*.*.*' not found` followed by
`<crate>: next version is <unchanged>`. The fix both times was the same
manual workaround (step 3 below) — this skill exists so that workaround
doesn't have to be rediscovered or manually driven every time.

## 0. Resolve the target crate and confirm the starting point

Confirm with the user (don't assume) if any of this is unclear from their
request:

- **Which crate** — must match a `crates/<name>` directory in this
  workspace.
- **Explicit version**, if they gave one (e.g. "release google-chat
  0.3.0"). If not given, you'll propose one in step 2.

Then:

```sh
git fetch origin main --tags --quiet
git status
```

If there are uncommitted changes or the current branch is behind
`origin/main`, sort that out before proceeding (per root CLAUDE.md's git
safety rules — don't discard uncommitted work, don't force anything). Any
branch this skill creates should be based on up-to-date `origin/main`, not a
stale local branch — checking out a branch that's already fully merged and
building on top of it (as happened earlier in this session) just creates
confusion.

## 1. Check there's actually something to release

```sh
git tag -l "<crate>-v*" | sort -V
```

If this returns nothing, this is the crate's first-ever release — skip to
step 4 (normal path); release-plz's first release isn't affected by the
registry-check quirk.

If tags exist, take the latest one and check for releasable commits since
then that touch this crate:

```sh
git log <crate>-vX.Y.Z..origin/main --oneline -- crates/<crate>/
```

If this is empty, there's nothing new to release — tell the user and stop
rather than inventing a version bump for no reason.

## 2. Check whether release-plz already opened a release PR

release-plz bundles all crates' pending bumps into one shared PR titled
`chore: release`:

```sh
gh pr list --search "chore: release" --state open
```

If one exists, open it (`gh pr view <number>`) and check whether it touches
`crates/<crate>/Cargo.toml`. If it does, this is the **normal path** — note
the version it proposes and skip to step 4 (merge), no workaround needed.
(This can legitimately happen even for a non-first release if something
about the timing or registry cache differs — don't assume the workaround is
always required, check first.)

If no such PR exists, or it exists but doesn't touch this crate, proceed to
step 3.

## 3. Workaround path: manual version bump

Decide the version:

- If the user gave an explicit version, use it (sanity-check it's valid
  semver and greater than the current `crates/<crate>/Cargo.toml` version).
- Otherwise, classify the commits found in step 1 by conventional-commit
  type — any `feat` → bump minor, only `fix`/`perf`/etc → bump patch, any
  commit body containing `BREAKING CHANGE` → bump major — and propose the
  resulting version to the user before proceeding. Don't silently pick one;
  confirm it, the same way a human releasing by hand would sanity-check the
  jump from 0.1.0 to 0.2.0 isn't accidentally a major bump.

Then, on a fresh branch off up-to-date `origin/main`:

```sh
git checkout -b bump-<crate>-version origin/main
# edit crates/<crate>/Cargo.toml: version = "<new version>"
cargo build -p <crate>   # refreshes Cargo.lock for the new version
git add crates/<crate>/Cargo.toml Cargo.lock
git commit -m "fix(<crate>): bump version to <new version> to work around release-plz registry check

release-plz's \"local version > registry version\" check skips auto-bumping
<crate> since this crate is never published (publish = false, registry
version effectively absent) — it would otherwise silently drop the
<short description of what's being released> from any release, same issue
and fix as 29e5ada (jira) and bc99eb2 (google-chat). Bumping by hand here so
release-plz picks up the existing commit(s) on the next run.

Co-Authored-By: Claude Sonnet 4.6 <noreply@anthropic.com>"
git push -u origin bump-<crate>-version
gh pr create --title "fix(<crate>): bump version to <new version> to work around release-plz registry check" --body "..."
```

The commit message must keep the `fix(<crate>)` scope — release-plz
attributes commits to crates by scope+files together, and this is the
established pattern from the two precedent commits.

## 4. Confirm and merge

**Always confirm with the user before merging** — this lands on `main` and
kicks off real CI/release infrastructure, the kind of visible/shared action
that needs a green light even when the rest of this skill runs
unsupervised. Tell them which PR, which version, and which path (normal or
workaround) before merging:

```sh
gh pr merge <number> --merge
```

(every PR merged in this repo so far is a regular merge commit, "Merge pull
request #N" — not squash or rebase; match that unless the user says
otherwise.)

Capture the resulting commit SHA on `main` — you'll need it to find the
right workflow run in the next step.

## 5. Watch the release-plz release job

```sh
gh run list --workflow=release-plz.yml --branch main --limit 3
```

Find the run triggered by the merge commit from step 4 (it may take a few
seconds to register — re-check rather than assuming it's missing). Watch it:

```sh
gh run watch <run-id> --exit-status
```

If it fails, don't just report "it failed" — pull the actual failing step's
log and quote the relevant lines:

```sh
gh run view <run-id> --log-failed
```

On success, confirm the tag was created:

```sh
git fetch --tags --quiet
git tag -l "<crate>-v<version>"
```

If the tag is missing despite the workflow succeeding, something about the
version comparison still didn't line up as expected (e.g. the version
bumped wasn't actually higher than the existing tag) — stop and surface this
to the user rather than guessing why.

## 6. Watch the binary build

The tag from step 5 triggers a second workflow, a 3-runner matrix
(`ubuntu-latest`, `ubuntu-24.04-arm`, `macos-latest`):

```sh
gh run list --workflow=release.yml --limit 3
```

Find the run for this tag and watch it the same way:

```sh
gh run watch <run-id> --exit-status
```

This is a matrix job — if it reports failure, identify *which* matrix leg(s)
failed (`gh run view <run-id>`) rather than treating it as one opaque
failure; one platform can fail while the others succeed, and the Release
will end up with fewer than 3 binaries attached.

## 7. Final verification — confirm the release actually exists

Don't take a green CI run as the final word — check the artifact it was
supposed to produce:

```sh
gh release view <crate>-v<version>
```

Confirm:
- The release exists and is **not** a draft.
- It has exactly three assets attached, named `<crate>-linux-x86_64`,
  `<crate>-linux-arm64`, `<crate>-macos-arm64` (per `release.yml`'s artifact
  naming).

If any asset is missing, that matrix leg from step 6 silently didn't attach
its binary even if the workflow run as a whole reported success — check that
leg's log specifically.

## 8. Final report

Summarize clearly:

- Crate and version released, and which path was used (normal release-plz
  PR, or the registry-check workaround).
- PR(s) opened/merged, with links.
- Tag and GitHub Release URL.
- Confirmation that all three platform binaries are attached (or which
  one(s) are missing and why, if step 7 found a gap).
- Anything that needed a judgment call (e.g. the version bump wasn't
  explicitly given by the user) so they can double-check it.
