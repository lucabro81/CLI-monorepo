---
name: release
description: Use when the user asks to release, ship, publish, or cut a version for one crate in this monorepo (e.g. "release jira", "fai una release di google-chat", "ship bitbucket 0.3.0"). Finds the auto-generated release PR for that crate, sanity-checks the version/changelog, merges it, and watches the tag + binary-build workflows through to a verified GitHub Release.
---

# Releasing a crate in this monorepo

Versioning is fully automatic. Every push to `main` runs
[`release-pr.yml`](../../../.github/workflows/release-pr.yml), which computes
each crate's next version from conventional commits via git-cliff (no
crates.io registry involved — see root `CLAUDE.md`'s "CI/CD" section for why
that matters here) and opens/updates a PR on a stable `release/<crate>`
branch (force-pushed each run, one PR per crate, auto-deleted on merge — no
branch clutter to clean up). Your job is **"find the PR, sanity-check it,
merge it, watch it flow through"** — not diagnosing why nothing happened.

This skill releases **one crate per invocation** — if the user wants
several, run it again for the next one.

## 1. Find the release PR

```sh
gh pr list --head "release/<crate>" --state open
```

If none exists, there are no releasable commits for that crate since its
last tag — confirm with the user before doing anything else. Options: wait
for the next push to `main` that touches `crates/<crate>/`, or manually
trigger the workflow:

```sh
gh workflow run release-pr.yml
```

## 2. Sanity-check before merging

Open the PR (`gh pr view <number>`) and confirm:
- The version bump level matches the commits included (`feat` → minor,
  `fix`/`perf` → patch, breaking change → major).
- The `CHANGELOG.md` diff looks reasonable (grouped under Added/Fixed/Other).

**Always confirm with the user before merging** — this lands on `main` and
kicks off real release infrastructure.

```sh
gh pr merge <number> --merge
```

(matches this repo's convention: regular merge commits, not squash/rebase.)

## 3. Watch the tag get created

```sh
gh run list --workflow=release-tag.yml --branch main --limit 3
gh run watch <run-id> --exit-status
git fetch --tags --quiet
git tag -l "<crate>-v<version>"
```

If the tag is missing despite the workflow succeeding, the crate's
`Cargo.toml` version on `main` didn't match what was expected — stop and
surface this rather than guessing why.

## 4. Watch the binary build

The tag triggers the existing 3-runner matrix build
([`release.yml`](../../../.github/workflows/release.yml), unchanged by the
git-cliff/cargo-release migration):

```sh
gh run list --workflow=release.yml --limit 3
gh run watch <run-id> --exit-status
```

If it fails, identify *which* matrix leg (`linux-x86_64`/`linux-arm64`/
`macos-arm64`) failed — one platform can fail while the others succeed.

## 5. Final verification

```sh
gh release view <crate>-v<version>
```

Confirm it's **not** a draft and has exactly three assets:
`<crate>-linux-x86_64`, `<crate>-linux-arm64`, `<crate>-macos-arm64`.

## 6. Final report

Summarize: crate and version released, PR link, tag, GitHub Release URL,
confirmation all three binaries are attached (or which are missing and why),
and anything that needed a judgment call.

## If something looks wrong on a run

This pipeline replaced release-plz, which had a structural bug for
`publish = false` crates (it queried crates.io to decide whether a bump was
needed, which silently never worked after a crate's first release — see
root `CLAUDE.md`'s CI/CD section for the full story). git-cliff has no
registry dependency, so that specific failure mode cannot recur. If the
computed version or changelog still looks wrong, check `cliff.toml` and each
crate's `[package.metadata.release]` pre-release-hook in its `Cargo.toml`
before assuming the underlying tools are broken — the previous manual
workaround process for the old bug is preserved in this file's git history
if it's ever needed as a reference, but should not be needed again.
