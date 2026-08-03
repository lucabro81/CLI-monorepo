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
    page.rs       — run(PageCommand); dispatches page get/create/update/search/delete;
                    also holds the --template-id/--body-file body-source
                    resolution logic (see "API design notes" below)
    space.rs      — run(SpaceCommand); dispatches space list
    template.rs   — run(TemplateCommand); dispatches template create/list/update/delete;
                    also holds the --body/--body-file resolution logic for
                    template create/update (resolve_body/resolve_optional_body
                    — narrower than page.rs's parse_body_source, no
                    --template-id equivalent)
  auth.rs         — thin wrapper over the shared `atlassian_auth` crate, fixing
                    this crate's config dir name and OAuth SCOPES: OAuthConfig,
                    Credentials, login(), login_client_credentials(), renew(),
                    save_credentials(), load_credentials(), path helpers,
                    get_granted_scopes(). The actual OAuth 2.0 (3LO + PKCE /
                    client_credentials) implementation, PKCE helpers, callback
                    parsing, and cloud_id resolution live in `atlassian_auth`
                    (workspace-local, shared with `jira` — see root CLAUDE.md's
                    "Shared library: crates/atlassian-auth" and BACKLOG.md's LIB-1)
  client.rs       — ConfluenceClient (blocking reqwest); get_json/post_json/put_json/delete
                    helpers; Confluence REST API methods spanning both API
                    generations [get_current_user, get_page, create_page,
                    update_page, delete_page, get_template, create_template,
                    update_template, delete_template, list_templates,
                    search_content, list_spaces]
  cli.rs          — clap structs: Cli (--select global), Command, AuthCommand,
                    PageCommand, SpaceCommand, TemplateCommand. No logic.
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
`--body`/`--body-file`/`--template-id` into exactly one content source)
and `validate_update_target` (the "at least one of --title/--body" runtime
check for `page update` — clap's `conflicts_with_all` only rules out passing
more than one body source on `create`, and there's no way to declare "at
least one of two optional flags" for `update` at all). `template.rs` has its
own `tests/commands/template_tests.rs` covering `resolve_body` (same
`--body`/`--body-file` resolution shape as `page.rs`'s, minus the
`--template-id` branch) and `resolve_optional_body` (the `template update`
variant — `None`/`None` is valid here, unlike `resolve_body`, since it means
"keep the current body" rather than "no content supplied"). `space.rs` has no dedicated unit-test file — its one
command is a thin passthrough, covered entirely by `cli_tests.rs`. `auth.rs`
is a thin wrapper over `atlassian_auth`
(see "OAuth / auth design" below) — its own test file only guards this
crate's config-dir name and SCOPES constant, not OAuth logic (covered by
`atlassian_auth`'s own tests). `doctor.rs` has no dedicated test file, same
reasoning as `jira`'s: every check either does live I/O or is trivially
correct status-string logic, nothing pure enough to isolate.

## OAuth / auth design

**Implementation lives in `atlassian-auth`** (workspace-local crate, `crates/atlassian-auth`), not in this crate — `auth.rs` here is a thin wrapper fixing the `confluence-cli` config dir name and this crate's `SCOPES` constant. `jira` uses the exact same underlying flows (same `auth.atlassian.com`/`api.atlassian.com` endpoints, same `cloud_id` resolution), just with its own scopes — see `BACKLOG.md`'s `LIB-1` for why this was extracted and `jira`'s own CLAUDE.md for the two grant types (`client_credentials` default, 3LO+PKCE via `--user`) and the Service Account vs 3LO-app tradeoffs, which apply identically here.

**`confluence init` is 3LO-app-only** (`commands/init.rs`), same as `jira init` — it always ends by launching the interactive browser consent flow, since that's what grants a 3LO app access to a site at all. A Service Account has no such step (access is assigned directly in admin.atlassian.com when the credential is created), so running `init` against one just hangs waiting for a browser flow that doesn't apply. Service Account setup skips `init` entirely: write `app.json` by hand, then `confluence auth login` directly. This is stated in `init`'s own `--help` text (`cli.rs`) and README Setup, not only here — it's the kind of thing worth surfacing at the point of use, not just in docs a caller has to already know to read.

**Scopes: `SCOPES`'s five Confluence entries are confirmed grantable and sufficient for `doctor`'s `api` check** — verified 2026-08-03 against a real Service Account credential (`admin.atlassian.com` → Directory → Service accounts → the credential's OAuth 2.0 scopes → Confluence product): `read:confluence-user`, `search:confluence`, `read:page:confluence`, `write:page:confluence`, `read:space:confluence` all showed up in `doctor`'s `oauth_scopes.granted` and `GET /wiki/rest/api/user/current` (the `api` check) succeeded. **Not yet exercised**: the actual `page get`/`create`/`update`/`search` and `space list` endpoints themselves — only the identity/scopes checks have been run live so far, not the core commands. Confluence Cloud's OAuth scope model mixes two incompatible sets that must not be combined *within a single API call's authorization check* (a classic-scoped token and a granular-scoped token are checked differently), but a single 3LO app/Service Account *can* request scopes from both sets at once — this crate does exactly that, because its commands span both Confluence API generations:

| Command | Endpoint | API version | Scope used |
|---|---|---|---|
| `auth whoami` | `GET /wiki/rest/api/user/current` | v1 | `read:confluence-user` (classic) — **live-verified** |
| `page search` | `GET /wiki/rest/api/content/search` | v1 | `search:confluence` (classic) — not yet exercised |
| `page get`/`create`/`update` | `/wiki/api/v2/pages...` | v2 | `read:page:confluence`/`write:page:confluence` (granular) — not yet exercised |
| `space list` | `GET /wiki/api/v2/spaces` | v2 | `read:space:confluence` (granular) — not yet exercised |

The v2 endpoints have **no classic-scope equivalent at all** per Atlassian's own docs — a classic-only scope grant cannot authorize them, regardless of how broad. If `doctor`'s `oauth_scopes` check passes but a specific command still 403s, the first thing to check is whether that command's scope (table above) was actually granted — not just "some scopes were granted."

**A Service Account credential shared with `jira`** (same `client_id`/`client_secret` in both `jira-cli/app.json` and `confluence-cli/app.json`, scoped to both products) works correctly — confirmed live. This exposed a real bug in the shared `atlassian-auth` crate, not specific to this setup: Atlassian's accessible-resources endpoint returns **multiple entries with the same site `id`** when a token's scopes span multiple products (one entry per product's scope subset, not their union), and `get_granted_scopes` originally only read the first matching entry — see `BACKLOG.md`'s `SCOPE-1` for the full story and fix. Fixed at the `atlassian-auth` level, so it's not something to work around per-crate.

## Config layout (XDG-style)

Both files live under `$XDG_CONFIG_HOME/confluence-cli/` (falling back to `~/.config/confluence-cli/`):

- `app.json` — `{"client_id": "...", "client_secret": "..."}`. Static; written by `confluence init` (3LO app path) or by hand (either path). Never modified at runtime.
- `credentials.json` — OAuth tokens. Fully managed by the CLI; never edit by hand.

Kept separate so automatic token writes never clobber the app identity — same reasoning as `jira`.

## API design notes

- **Two API generations, one client**: Confluence Cloud exposes v2 (`/wiki/api/v2/...`, the current actively-developed surface, cursor-based pagination) alongside v1 (`/wiki/rest/api/...`, offset-based pagination) under the same site. This crate uses v2 wherever a command has a v2 endpoint (`page get`/`create`/`update`, `space list`) and falls back to v1 only where v2 has no equivalent yet (`auth whoami`'s current-user check, `page search`'s CQL search, all of `template`). `endpoints.rs` documents which constant belongs to which version.
- **`page get`** always requests `?body-format=storage` — v2's page GET omits the body entirely unless a `body-format` is explicitly requested, and every other command in this crate (create/update body resolution, `page get` itself) works in storage format, so there's no reason to expose a `--body-format` flag until a concrete need for `atlas_doc_format` or another representation comes up.
- **`page create --template-id`/`--body`/`--body-file`**: Confluence has no API to create a page "from" a template in one call — that specific gap is confirmed (Atlassian Community threads; an undocumented `contentBlueprintSpec` workaround exists but is deliberately not used here, since undocumented APIs can change without notice). This is narrower than "no template API at all": `POST /wiki/rest/api/template` genuinely exists and creates a content template object (confirmed against developer.atlassian.com — requires space Admin permission or Confluence Administrator for a global template; implemented as `template create`, see below). What's missing is specifically the shortcut from "template" to "populated page" — so `--template-id` fetches an *existing* template's stored body via `GET /wiki/rest/api/template/{id}` (`body.storage.value`) and submits it as the new page's initial content, functionally identical to duplicating the template by hand. `--body`/`--body-file` are the non-template path: raw content, either inline or read from a local file — `--body-file` is deliberately *not* named `--template-file`, since it has no relation to Confluence's Template feature at all, just a convenience for longer content. `--body`/`--body-file`/`--template-id` are mutually exclusive (clap `conflicts_with_all`) and exactly one is required (`parse_body_source`, `commands/page.rs`, runtime-checked and unit-tested since clap can't express "exactly one of three" declaratively for enum-variant fields).
- **`template create`**: deliberately a separate command from `page create`, not a side effect of `page create --body-file` — giving one flag two distinct, non-obvious effects (create a page *and* silently register a template) was considered and rejected during design discussion 2026-08-03. `POST /wiki/rest/api/template` requires `name`, `templateType` (always `"page"` here — Confluence also has non-page template types this crate doesn't create), and `body` (same `{representation, value}` shape as page bodies); `space` is optional — omit it for a global template (Confluence Administrator permission), include `--space-key` for a space template (Admin permission on that space). Content resolution (`--body`/`--body-file`, exactly one required) is `resolve_body` in `commands/template.rs` — a narrower version of `page.rs`'s `parse_body_source` with no `--template-id`-equivalent third source, since a template referencing another template isn't a supported concept. The created template's `templateId` composes directly with `page create --template-id`.
- **`template list`**: `GET /wiki/rest/api/template/page` (page-type templates specifically; Confluence also has a separate `/template/blueprint` listing not implemented here), offset pagination (`--start`/`--limit`, default 25), optional `--space-key` to scope to one space instead of listing global templates.
- **`page update`**: Confluence v2 has no partial-patch endpoint — every `PUT /wiki/api/v2/pages/{id}` must submit the full page (title, body, version). This command fetches the current page first (reusing `get_page`) to learn its current title/body/version, then submits a full update with `--title`/`--body` overriding just those fields and `version.number` incremented by exactly one. At least one of `--title`/`--body` is required (`validate_update_target`) — otherwise the "update" would just resubmit unchanged content and bump the version number for no reason.
- **`page search`**: `GET /wiki/rest/api/content/search` with a raw CQL string via `--cql`, offset pagination (`--start`/`--limit`, default 25). No CQL validation/building on the client side — same "pass the query language straight through" approach as `jira issue search --jql`.
- **`page delete --purge`**: `DELETE /wiki/api/v2/pages/{id}` defaults to moving the page to Confluence's trash (recoverable) — not a permanent delete, unlike every other `--confirm`-gated delete in this workspace so far (`jira issue delete`, `bitbucket repo delete`). `?purge=true` requests permanent removal, but per Atlassian's own docs this only works on a page that's *already* trashed — a caller wanting a true one-shot permanent delete must call this command twice (plain, then `--purge`). Not yet live-verified what `purge=true` does against a non-trashed page (error vs. silently trashing it) — see "Known gaps". `client.rs`'s `delete_page` and the new `delete` helper expect Confluence's 204 No Content and return `Ok(())`, unlike every other client method here which returns the parsed response body — there is none to parse.
- **Page position/ordering is read-only** — `page get`'s response includes a `position` field (a page's order among siblings, visible today via `--select position` with no extra code needed), but there is no documented Confluence Cloud endpoint to *change* it. Confirmed via the still-open Atlassian feature request [CONFCLOUD-40101](https://jira.atlassian.com/browse/CONFCLOUD-40101) ("Gathering Interest", not implemented) — the UI's drag-and-drop reorder uses something internal/undocumented. No `page move`/reorder command exists in this crate because there's nothing in the public API for it to call; revisit only if Atlassian ships this.
- **`space list`**: `GET /wiki/api/v2/spaces`, cursor pagination (`--cursor`, from the previous response's `_links.next`) — v2's pagination style, distinct from `page search`'s v1 offset style. No `space get` yet; add one when a concrete need for fetching a single space by ID/key arises (root CLAUDE.md's incremental-build rule).
- **`template update`**: same "no partial-patch" shape as `page update`, but `PUT /wiki/rest/api/template` is *not* ID-scoped in the URL the way `PUT /pages/{id}` is — `templateId` goes in the request body instead (`client.rs`'s `update_template` PUTs to the bare `PATH_TEMPLATE`, not a per-ID path — see `template_path` vs `PATH_TEMPLATE`). Fetches the current template via the existing `get_template` to fill in `name`/`templateType`/`body` (and `space.key`, if the fetched template has one) for whichever of `--name`/`--description`/`--body`(-file) weren't overridden. At least one of those four is required (`TemplateUpdateMissingTarget`) — same reasoning as `page update`. Not yet live-verified whether the `GET /wiki/rest/api/template/{id}` response actually includes a `space` object to read back (`current["space"]["key"]` is read defensively — if absent, `space` is simply omitted from the update body rather than assumed).
- **`template delete`**: `DELETE /wiki/rest/api/template/{id}`, 204 No Content. Unlike `page delete`, Confluence's own docs describe content-template deletion as immediate and permanent — no trash/`--purge` distinction, so this command has only `--confirm`.
- **`--select`/`--select-all`** (global flags, see root `CLAUDE.md`): `--select` is mandatory by default. Exempt commands (always print in full via `select.or_all()`) and why:
  | Command | Exempt? | Why |
  |---|---|---|
  | `doctor` | yes | internally-generated report, fixed/small |
  | `auth whoami` | yes | identity check, fixed/small |
  | `page get` | **no** | page bodies are arbitrary-length content, can be large |
  | `page create` | **no** | response echoes back the full created page including body — same size risk as `page get`, unlike jira's `issue create` (which returns only `{id,key,self}`) |
  | `page update` | **no** | same reasoning as `page create` |
  | `page search` | **no** | list endpoint, unbounded |
  | `page delete` | yes | synthesized by us: `{"deleted": true, "id": ..., "purged": bool}` — Confluence itself returns 204 No Content |
  | `space list` | **no** | list endpoint, unbounded |
  | `template create` | **no** | response echoes back the full created template including body — same reasoning as `page create` |
  | `template list` | **no** | list endpoint, unbounded |
  | `template update` | **no** | same reasoning as `page create`/`page update` |
  | `template delete` | yes | synthesized by us: `{"deleted": true, "id": ...}` — Confluence itself returns 204 No Content |

## Known gaps

- **No e2e tests yet.** Login, `doctor`, and `auth whoami` have been verified live (see "OAuth / auth design" above), but `page get`/`create`/`update`/`search`/`delete`, `space list`, and `template create`/`list`/`update`/`delete` have only been verified against `--help` output, clap parsing, and unit-testable pure logic — not against a real Confluence site. Add `tests/e2e_tests.rs` (mirroring `jira`'s `IssueGuard`-style pattern: a `PageGuard`/`TemplateGuard` that deletes/removes resources created during tests) the first time these commands are exercised live — see the `add-confluence-command` skill's `ADDENDUM.md` for the live-test target once one is chosen.
- **No `doctor` permission-scheme layer** — unlike `jira`'s `service_user`/`projects` checks (which walk Jira's `mypermissions` API per-project), `confluence doctor` stops at `oauth_scopes`. Confluence's space-permission model is a different API surface; add a check here once a command actually needs to distinguish "token has the right OAuth scope" from "this account has permission in this specific space." This also applies to `template create`/`update`/`delete`'s Admin/Confluence-Administrator permission requirement, distinct from any OAuth scope.
- **`page delete --purge` against a non-trashed page is unverified** — see "API design notes" above.
- **`template update`'s `space` carry-over is unverified** — whether `GET /wiki/rest/api/template/{id}` actually returns a `space` object to read back is not yet confirmed live; see "API design notes" above.

## Planned commands (build incrementally, smallest first)

Candidates for the next concrete need, not committed yet: `space get <id-or-key>`, attachment upload/download, page label management. Follow root `CLAUDE.md`'s incremental-build rule — don't build these speculatively. **Not planned**: page reordering/move — Confluence Cloud has no public API for it (see "API design notes" above), so there's nothing to build against.
