# CLAUDE.md — crates/confluence

Architecture and design notes for the `confluence` crate. Global rules (TDD, error handling, flag conventions, commands) are in the root `CLAUDE.md`.

## Module map

```
src/
  commands/
    mod.rs        — pub mod declarations for all command handlers
    auth.rs       — run_login(), run_whoami()
    doctor.rs     — run_doctor(); also called by init as final verification
    init.rs       — run_init(), write_app_config(); human onboarding flow
    page.rs       — run(PageCommand); dispatches page get/create/update/search;
                    also holds the --template-id/--template-file body-source
                    resolution logic (see "API design notes" below)
    space.rs      — run(SpaceCommand); dispatches space list
  auth.rs         — thin wrapper over the shared `atlassian_auth` crate, fixing
                    this crate's config dir name and OAuth SCOPES: OAuthConfig,
                    Credentials, login(), login_client_credentials(), renew(),
                    save_credentials(), load_credentials(), path helpers,
                    get_granted_scopes(). The actual OAuth 2.0 (3LO + PKCE /
                    client_credentials) implementation, PKCE helpers, callback
                    parsing, and cloud_id resolution live in `atlassian_auth`
                    (workspace-local, shared with `jira` — see root CLAUDE.md's
                    "Shared library: crates/atlassian-auth" and BACKLOG.md's LIB-1)
  client.rs       — ConfluenceClient (blocking reqwest); get_json/post_json/put_json
                    helpers; Confluence REST API methods spanning both API
                    generations [get_current_user, get_page, create_page,
                    update_page, get_template, search_content, list_spaces]
  cli.rs          — clap structs: Cli (--select global), Command, AuthCommand,
                    PageCommand, SpaceCommand. No logic.
  context.rs      — config_dir(), load_oauth_config(), authenticated_client(),
                    print_json(value, select), client_error_to_cli(e). Shared
                    by all command handlers.
  endpoints.rs    — URL/path constants for the Confluence Cloud REST API
                    (both v1 and v2), used by client.rs. No logic. Atlassian
                    OAuth endpoints (used by auth.rs) live in
                    `atlassian_auth::endpoints` instead (shared with jira).
  error.rs        — CliError (top-level, thiserror-derived), including a
                    transparent Select variant wrapping cli_fields::RenderError.
  tests/          — all *_tests.rs files, mirroring the src/ layout (see "Test
                    file convention" below). No e2e_tests.rs yet — see
                    "Known gaps" below.
  main.rs         — pure dispatch: resolve --select/--select-all into a
                    cli_fields::Select once, match Command, call commands::*.
```

`--select` dot-notation projection itself (`filter_fields`, `describe_top_level_shape`, the `Select` enum, `render_json`) lives in the shared `crates/cli-fields` workspace crate, not in this crate — see root `CLAUDE.md`'s "Shared library: crates/cli-fields".

## Test file convention

See root `CLAUDE.md` for the general `src/tests/` convention and the
cli_tests/commands split. `page.rs` has a dedicated
`tests/commands/page_tests.rs` covering `parse_body_source` (resolves
`--body`/`--template-file`/`--template-id` into exactly one content source)
and `validate_update_target` (the "at least one of --title/--body" runtime
check for `page update` — clap's `conflicts_with_all` only rules out passing
more than one body source on `create`, and there's no way to declare "at
least one of two optional flags" for `update` at all). `space.rs` has no
dedicated unit-test file — its one command is a thin passthrough, covered
entirely by `cli_tests.rs`. `auth.rs` is a thin wrapper over `atlassian_auth`
(see "OAuth / auth design" below) — its own test file only guards this
crate's config-dir name and SCOPES constant, not OAuth logic (covered by
`atlassian_auth`'s own tests). `doctor.rs` has no dedicated test file, same
reasoning as `jira`'s: every check either does live I/O or is trivially
correct status-string logic, nothing pure enough to isolate.

## OAuth / auth design

**Implementation lives in `atlassian-auth`** (workspace-local crate, `crates/atlassian-auth`), not in this crate — `auth.rs` here is a thin wrapper fixing the `confluence-cli` config dir name and this crate's `SCOPES` constant. `jira` uses the exact same underlying flows (same `auth.atlassian.com`/`api.atlassian.com` endpoints, same `cloud_id` resolution), just with its own scopes — see `BACKLOG.md`'s `LIB-1` for why this was extracted and `jira`'s own CLAUDE.md for the two grant types (`client_credentials` default, 3LO+PKCE via `--user`) and the Service Account vs 3LO-app tradeoffs, which apply identically here.

**`confluence init` is 3LO-app-only** (`commands/init.rs`), same as `jira init` — it always ends by launching the interactive browser consent flow, since that's what grants a 3LO app access to a site at all. A Service Account has no such step (access is assigned directly in admin.atlassian.com when the credential is created), so running `init` against one just hangs waiting for a browser flow that doesn't apply. Service Account setup skips `init` entirely: write `app.json` by hand, then `confluence auth login` directly. This is stated in `init`'s own `--help` text (`cli.rs`) and README Setup, not only here — it's the kind of thing worth surfacing at the point of use, not just in docs a caller has to already know to read.

**Scopes are not yet live-verified** (see `auth.rs`'s `SCOPES` doc comment) — this is the one meaningful way this crate's auth design differs in confidence level from `jira`'s (whose scopes were verified end-to-end against a real org). Confluence Cloud's OAuth scope model mixes two incompatible sets that must not be combined *within a single API call's authorization check* (a classic-scoped token and a granular-scoped token are checked differently), but a single 3LO app/Service Account *can* request scopes from both sets at once — this crate does exactly that, because its commands span both Confluence API generations:

| Command | Endpoint | API version | Scope used |
|---|---|---|---|
| `auth whoami` | `GET /wiki/rest/api/user/current` | v1 | `read:confluence-user` (classic) |
| `page search` | `GET /wiki/rest/api/content/search` | v1 | `search:confluence` (classic) |
| `page get`/`create`/`update` | `/wiki/api/v2/pages...` | v2 | `read:page:confluence`/`write:page:confluence` (granular) |
| `space list` | `GET /wiki/api/v2/spaces` | v2 | `read:space:confluence` (granular) |

The v2 endpoints have **no classic-scope equivalent at all** per Atlassian's own docs — a classic-only scope grant cannot authorize them, regardless of how broad. If `doctor`'s `oauth_scopes` check passes but a specific command still 403s, the first thing to check is whether that command's scope (table above) was actually granted — not just "some scopes were granted." Confirm/refine this table against [developer.atlassian.com/cloud/confluence/scopes-for-oauth-2-3LO-and-forge-apps](https://developer.atlassian.com/cloud/confluence/scopes-for-oauth-2-3LO-and-forge-apps/) and live testing the first time each command is actually exercised end-to-end.

## Config layout (XDG-style)

Both files live under `$XDG_CONFIG_HOME/confluence-cli/` (falling back to `~/.config/confluence-cli/`):

- `app.json` — `{"client_id": "...", "client_secret": "..."}`. Static; written by `confluence init` (3LO app path) or by hand (either path). Never modified at runtime.
- `credentials.json` — OAuth tokens. Fully managed by the CLI; never edit by hand.

Kept separate so automatic token writes never clobber the app identity — same reasoning as `jira`.

## API design notes

- **Two API generations, one client**: Confluence Cloud exposes v2 (`/wiki/api/v2/...`, the current actively-developed surface, cursor-based pagination) alongside v1 (`/wiki/rest/api/...`, offset-based pagination) under the same site. This crate uses v2 wherever a command has a v2 endpoint (`page get`/`create`/`update`, `space list`) and falls back to v1 only where v2 has no equivalent yet (`auth whoami`'s current-user check, `page search`'s CQL search, template lookup). `endpoints.rs` documents which constant belongs to which version.
- **`page get`** always requests `?body-format=storage` — v2's page GET omits the body entirely unless a `body-format` is explicitly requested, and every other command in this crate (create/update body resolution, `page get` itself) works in storage format, so there's no reason to expose a `--body-format` flag until a concrete need for `atlas_doc_format` or another representation comes up.
- **`page create --template-id`/`--template-file`**: Confluence has no API to create a page "from" a template directly (confirmed via Atlassian Community threads, not officially documented — an undocumented `contentBlueprintSpec` workaround exists but is deliberately not used here, since undocumented APIs can change without notice). `--template-id` instead fetches the template's stored body via `GET /wiki/rest/api/template/{id}` (`body.storage.value`) and submits it as the new page's initial content — functionally identical to duplicating the template by hand. `--template-file` does the same from a local file instead of a live template. `--body`/`--template-file`/`--template-id` are mutually exclusive (clap `conflicts_with_all`) and exactly one is required (`parse_body_source`, `commands/page.rs`, runtime-checked and unit-tested since clap can't express "exactly one of three" declaratively for enum-variant fields).
- **`page update`**: Confluence v2 has no partial-patch endpoint — every `PUT /wiki/api/v2/pages/{id}` must submit the full page (title, body, version). This command fetches the current page first (reusing `get_page`) to learn its current title/body/version, then submits a full update with `--title`/`--body` overriding just those fields and `version.number` incremented by exactly one. At least one of `--title`/`--body` is required (`validate_update_target`) — otherwise the "update" would just resubmit unchanged content and bump the version number for no reason.
- **`page search`**: `GET /wiki/rest/api/content/search` with a raw CQL string via `--cql`, offset pagination (`--start`/`--limit`, default 25). No CQL validation/building on the client side — same "pass the query language straight through" approach as `jira issue search --jql`.
- **`space list`**: `GET /wiki/api/v2/spaces`, cursor pagination (`--cursor`, from the previous response's `_links.next`) — v2's pagination style, distinct from `page search`'s v1 offset style. No `space get` yet; add one when a concrete need for fetching a single space by ID/key arises (root CLAUDE.md's incremental-build rule).
- **`--select`/`--select-all`** (global flags, see root `CLAUDE.md`): `--select` is mandatory by default. Exempt commands (always print in full via `select.or_all()`) and why:
  | Command | Exempt? | Why |
  |---|---|---|
  | `doctor` | yes | internally-generated report, fixed/small |
  | `auth whoami` | yes | identity check, fixed/small |
  | `page get` | **no** | page bodies are arbitrary-length content, can be large |
  | `page create` | **no** | response echoes back the full created page including body — same size risk as `page get`, unlike jira's `issue create` (which returns only `{id,key,self}`) |
  | `page update` | **no** | same reasoning as `page create` |
  | `page search` | **no** | list endpoint, unbounded |
  | `space list` | **no** | list endpoint, unbounded |

## Known gaps

- **No e2e tests yet.** Every command here has been verified against `--help` output, clap parsing, and unit-testable pure logic, but not against a real Confluence site — this crate has not yet had a live login performed against it. Add `tests/e2e_tests.rs` (mirroring `jira`'s `IssueGuard`-style pattern: a `PageGuard` that deletes/trashes pages created during tests) the first time this crate is exercised against a real site — see the `add-confluence-command` skill's `ADDENDUM.md` for the live-test target once one is chosen.
- **Scopes need live confirmation** — see "OAuth / auth design" above.
- **No `doctor` permission-scheme layer** — unlike `jira`'s `service_user`/`projects` checks (which walk Jira's `mypermissions` API per-project), `confluence doctor` stops at `oauth_scopes`. Confluence's space-permission model is a different API surface; add a check here once a command actually needs to distinguish "token has the right OAuth scope" from "this account has permission in this specific space."

## Planned commands (build incrementally, smallest first)

Candidates for the next concrete need, not committed yet: `space get <id-or-key>`, `page delete`, attachment upload/download, page label management. Follow root `CLAUDE.md`'s incremental-build rule — don't build these speculatively.
