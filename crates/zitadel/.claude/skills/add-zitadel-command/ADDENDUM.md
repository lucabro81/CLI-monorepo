# `zitadel` crate specifics for add-cli-command

Read alongside `.claude/skills/add-cli-command/SKILL.md` (workspace root) and
`crates/zitadel/CLAUDE.md` (module map, auth design, API notes, command table —
not repeated here).

Section headings match `SKILL.md`'s step numbers — only steps where this crate
deviates from or adds to the generic skill are covered. Steps not listed follow
`SKILL.md` as-is.

## Step 1 — Scope

- **Permission check**: ZITADEL authorizes by **manager roles** on the calling
  identity (IAM_OWNER, IAM_ORG_MANAGER, ORG_OWNER, ORG_USER_MANAGER,
  PROJECT_OWNER, …), not by OAuth scopes — every API token carries the same
  `urn:zitadel:iam:org:project:id:zitadel:aud` scope. For a new command, find the
  required role/permission in the endpoint's API reference and note it in the
  command's `--help` and README section. A 403 should map to an error naming
  that role and pointing to `zitadel doctor`.
- No new OAuth scope is normally needed; if one is, add it to the scope constants
  in `auth.rs` and re-run `auth login`.

## Step 2 — API research

- Docs: `https://zitadel.com/docs/apis/introduction` (overview) and the per-service
  references under `https://zitadel.com/docs/apis/resources/` (use WebFetch).
- Prefer the **v2** REST JSON endpoint (`/v2/...`). Use v1 (`/management/v1`,
  `/auth/v1`, `/admin/v1`) only if v2 has no equivalent, and say so in the
  crate CLAUDE.md command table. v1 Management calls take the target org via the
  `x-zitadel-orgid` header.
- Searches are `POST` with `{"query":{"offset","limit","asc"},"queries":[...]}`;
  expose `--limit`/`--offset` and pass the raw response (incl. `details.totalResult`) through.

## Step 6 — e2e tests

`crates/zitadel/src/tests/e2e_tests.rs`, each test `#[ignore = "e2e: requires zitadel init"]`.

- **Target**: the maintainer's own ZITADEL Cloud free-tier instance, configured
  via `zitadel init` in the config dir and `ZITADEL_E2E_*` variables in the
  workspace-root `.env` (never commit the instance URL or ids).
- **Mutations allowed**: the instance may be freely mutated as long as usage
  stays within the free tier. Tests may create their own fixtures, named with
  the prefix `zitadel-cli-e2e-` + timestamp, and must remove them on drop (RAII
  guard, as jira's `IssueGuard`), plus an `e2e_cleanup` test that deletes
  leftovers matching the prefix.
- **Running**:
  ```sh
  cargo test -p zitadel -- --ignored --test-threads=1
  ```
- Assert response shape (top-level keys/types) rather than exact content, except
  for fixtures the test created itself.

## GitHub issue scope label

Use the `zitadel` label (per root `CLAUDE.md`'s "Tracking known issues and design notes").
