# `bitbucket` crate specifics for add-cli-command

Read alongside `.claude/skills/add-cli-command/SKILL.md` (workspace root).
This file has no frontmatter — it is not an invocable skill on its own, just
reference material the root skill reads.

## Step 1 — Scope

- **Command groups**: `RepoCommand`, `AuthCommand`, `PrCommand`.
- **BACKLOG ID prefixes**: `REPO-*`, `AUTH-*`, `PR-*`, `LIB-1`, `DOCTOR-1`
  (and others — grep `BACKLOG.md` for `(bitbucket)`).
- **Inputs precedent**: typed flags, one per field (`repo create`), unless
  the field count is large/open-ended (see `REPO-1` for when a raw JSON body
  might make more sense instead).
- **Destructive-command precedent**: explicit `--confirm` flag, no
  interactive prompt, error message includes the exact retry command (no
  command implements this yet — `pr decline`/`pr merge`/`repo delete` will).
- **Auth/scope system**: OAuth scopes are granted directly to the workspace
  OAuth consumer at creation time (Settings → OAuth consumers → edit). Check
  `bitbucket doctor`'s `permissions.granted_scopes` against Bitbucket's
  documented scope for the endpoint (see step 2). If a new scope is needed,
  flag it explicitly — adding it to the consumer is a one-time human step. Do
  **not** add a new key to `doctor`'s permissions check for this — per
  `DOCTOR-1`, it reports the granted-scopes list as-is and is not matched
  against a fixed "required scopes" list.

## Step 2 — API research

- Docs: `https://developer.atlassian.com/cloud/bitbucket/rest/`
  (use WebFetch/WebSearch).
- **Pagination**: page-number based (`?page=N`, response includes `page`,
  `pagelen`, `size`, and a `next` URL) — see `repo list` /
  `endpoints::path_repositories` for the pattern. Not cursor-based.

## Live test target

Workspace `lucabrognaracode`, repos `lucabrognaracode/repo-test` and
`lucabrognaracode/cli-test-repo`. Ask the user if a different
workspace/repo slug is needed.

## Step 6 — e2e tests

None. This crate has no automated e2e suite — step 5's manual live
verification against the real workspace is the only check beyond unit tests.

## Step 7 — verification loop command

```sh
cargo test -p bitbucket
cargo clippy -p bitbucket --all-targets
```
