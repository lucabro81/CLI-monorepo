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
  in `endpoints.rs` (`SERVICE_USER_SCOPES`, and the `--user <USER_ID>` scopes) and re-run `auth login` (and `auth login --user <USER_ID>` for each person).

## Step 2 — API research

- Docs: `https://zitadel.com/docs/apis/introduction` (overview) and the per-service
  references under `https://zitadel.com/docs/apis/resources/` (use WebFetch).
- Prefer the **v2** REST JSON endpoint (`/v2/...`). Use v1 (`/management/v1`,
  `/auth/v1`, `/admin/v1`) only if v2 has no equivalent, and say so in the
  crate CLAUDE.md command table. v1 Management calls take the target org via the
  `x-zitadel-orgid` header.
- Two request/response shapes coexist — probe before assuming:
  - user/organization services (REST `/v2/...`): `POST` with
    `{"query":{"offset","limit","asc"},"queries":[...]}`, `TEXT_QUERY_METHOD_*`;
    response `details.totalResult` + `result`.
  - newer services without a REST mapping (e.g. `ProjectService`): `POST
    /zitadel.<service>.v2.<Service>/<Method>` (Connect path, plain
    `application/json`), body `{"pagination":{...},"filters":[...]}`,
    `TEXT_FILTER_METHOD_*`; response `pagination.totalResult` + a named list.
  - Either way: build the pagination block with `context::search_query`, expose
    `--limit`/`--offset`, pass the raw response through.
- Validate every enum filter client-side (clap `ValueEnum`): ZITADEL answers an
  unknown enum value with 200 and zero results instead of an error.

## Step 6 — e2e tests

`crates/zitadel/src/tests/e2e_tests.rs`, each test `#[ignore = "e2e: requires zitadel init"]`.

- **Target**: the maintainer's own ZITADEL Cloud free-tier instance, configured
  via `zitadel init` in the config dir in use (`XDG_CONFIG_HOME`; never commit
  the instance URL or ids). No env variables are needed.
- **Self-fixture for read-only commands**: `setup()` returns the logged-in
  identity (user id, username, organization from `GET /auth/v1/users/me`) —
  assert that read/search/list commands find *it*, instead of creating data.
- **Mutations allowed** (for future write commands): the instance may be freely mutated as long as usage
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
