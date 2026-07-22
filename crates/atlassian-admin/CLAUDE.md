# CLAUDE.md — crates/atlassian-admin

Architecture and design notes for the `atlassian-admin` crate. Global rules (TDD, error handling, flag conventions, commands) are in the root `CLAUDE.md`.

## Status

`init`, `doctor`, `user get` implemented. This is the crate's full planned command pool for now — new commands land as concrete needs arise (per root CLAUDE.md's incremental approach).

## Module map

```
src/
  commands/
    mod.rs        — pub mod declarations for all command handlers
    doctor.rs     — run_doctor(); also called by init as final verification
    init.rs       — run_init(), write_app_config(); human onboarding flow
    user.rs       — run(UserCommand); dispatches user subcommands [get]
  auth.rs         — AdminConfig (api_key, org_id), load()/path helpers. No
                    token exchange, no expiry, no refresh — the API key from
                    app.json is used directly as the Bearer token on every
                    call. No credentials.json: there is nothing dynamic to
                    persist (see "Auth design" below for why this crate has
                    no `auth login`/`auth whoami` commands, unlike every
                    other crate).
  client.rs       — AdminClient (blocking reqwest); get_json helper;
                    Organization Admin API methods [get_organization,
                    get_user]
  cli.rs          — clap structs: Cli (--select global), Command,
                    UserCommand. No logic.
  context.rs      — config_dir(), authenticated_client(), print_json(value, select)
  endpoints.rs    — URL/path constants for the Atlassian Organization Admin API.
  error.rs        — CliError (top-level, thiserror-derived), including a
                    transparent Select variant wrapping cli_fields::RenderError.
  tests/          — all *_tests.rs files, mirroring the src/ layout (see root
                    CLAUDE.md's "Test file convention").
  main.rs         — pure dispatch: resolve --select/--select-all into a
                    cli_fields::Select once, match Command, call commands::*.
```

`--select` dot-notation projection itself (`filter_fields`, `describe_top_level_shape`, the `Select` enum, `render_json`) lives in the shared `crates/cli-fields` workspace crate, not in this crate — see root `CLAUDE.md`'s "Shared library: crates/cli-fields".

## Test file convention

See root `CLAUDE.md` for the general `src/tests/` convention and the
cli_tests/commands split. `doctor.rs` and `user.rs` are thin passthrough
modules with no dedicated `tests/commands/` file — `doctor.rs` has no
network-free pure logic to isolate (unlike bitbucket's `check_permissions`),
and `user.rs` has no body-building/identifier-splitting logic at all; both
are covered entirely by `cli_tests.rs` plus manual live verification.
`init.rs` keeps `write_app_config` unit-tested (filesystem-only, no network)
but `run_init`'s branching (direct write vs skeleton) is verified manually —
`config_dir()` reads `$XDG_CONFIG_HOME` directly and isn't parameterized for
injection, matching every other crate's `init` test coverage.

## Auth design

**Not OAuth — a static API key.** The Atlassian Organization Admin API
(`api.atlassian.com/admin/v1/orgs/...`) is a distinct product surface from
Jira/Confluence/Bitbucket: it authenticates with a long-lived **Organization
API key**, created by an org admin at admin.atlassian.com (Organization
settings → API keys), sent as a plain Bearer token — no OAuth grant, no
consumer, no client_id/secret pair, no expiry/refresh cycle to manage.

- **Key creation** (one-time, requires org-admin/superadmin access): admin.atlassian.com → select the organization → **Organization settings → API keys** → **Create API key**. **Use "API keys without scopes"** (full access) — required for `user get`, see "Scopes" below; "with scopes" cannot satisfy it. Name it, set an expiration (max 1 year — must be manually rotated before then), **Create**. The confirmation screen shows the **Organization ID** and the **API key together, once** — copy both immediately, they cannot be recovered afterward. Scopes cannot be edited on an existing key afterward — changing them means revoking the key and creating a new one.
- **Scopes**: `user get` (`GET /users/{account_id}/manage/profile`) needs a scope that shows up in live 403 responses as `manage:org` (also `manage/org/public-api`, `manage:me:DUMMYSCOPE`) but **does not appear in Atlassian's public scope catalog** (developer.atlassian.com/cloud/admin/scopes) — confirmed not selectable when creating a scoped ("with scopes") key. Only an **unscoped ("without scopes")** key reaches it (confirmed live 2026-07-22). `doctor`'s `api` check (`GET /admin/v1/orgs/{org_id}`) separately needs `read:orgs:admin`, which *is* in the public catalog and *is* selectable — but since `user get` requires the unscoped key anyway, there's no reason to also select it; an unscoped key covers both. **`read:accounts:admin`/`read:directories:admin` (this crate's originally documented/selected scopes, before any live verification) grant neither endpoint actually used** — see "Corrections found via live testing" below.
- **Managed accounts only**: the API only resolves accounts whose email domain is verified/claimed under this organization (via Atlassian Access/Guard). An external Atlassian account (personal ID, unrelated domain) is invisible to this API regardless of scope.
- **Why this is its own crate, not folded into `jira` or `bitbucket`**: this is a genuinely different service (different host, different auth model, different product) from either — see root `CLAUDE.md`'s "one crate per external service" convention. It was *not* a case for `jira`/`bitbucket`'s deferred shared-Atlassian-auth-library discussion (`LIB-1` in `BACKLOG.md`) either, since that library is specifically about the OAuth patterns those two crates duplicate — this crate has no OAuth at all.
- **Why no `auth login`/`auth whoami` commands, unlike every other crate in this workspace**: every other crate's baseline command set includes these because there's a real exchange step (OAuth grant → access token) and a real "who am I" identity endpoint to call. Neither exists here: the API key from `app.json` *is* the finished credential the moment `init` writes it — there is nothing to exchange, so "login" would be a no-op command. And the Organization Admin API has no endpoint that identifies "the caller" of an API key (it identifies whichever `account_id` you *ask about*, not the key itself) — so there's no meaningful "whoami" to build. `doctor`'s `api` check (a live `GET /v1/orgs/{org_id}` call) is the auth-sanity-check in place of both.

Config layout, mirroring other crates (`$XDG_CONFIG_HOME/atlassian-admin-cli/`, falling back to `~/.config/atlassian-admin-cli/`):

- `app.json` — `{"api_key": "...", "org_id": "..."}`. Static, hand-written or written by `init`. The CLI never modifies it at runtime.
- No `credentials.json` — there is no dynamic token to persist; `app.json` alone is sufficient to authenticate every call.

### `init`'s non-interactive design (deliberate deviation from jira/bitbucket/google-chat's `init`)

Every other crate's `init` falls back to an interactive stdin prompt for any credential not passed via flags. This crate's `init` does **not** prompt for `--api-key` on stdin, by design: it is a long-lived, org-wide-privileged secret (far more powerful than a single-product OAuth consumer), and typing it into an interactive terminal risks it landing in scrollback buffers, terminal session recordings, or tmux/screen logs in a way a quick paste into a file editor does not.

- `init --api-key <KEY> --org-id <ID>` — both provided → writes `app.json` directly, same as other crates.
- `init` (either flag omitted) — writes `app.json` as a skeleton (empty `api_key`/`org_id` string fields) if it doesn't already exist, and prints the exact path to paste the real values into by hand. No stdin prompt, no `doctor` auto-run afterward (unlike other crates' `init`, which chains straight into a live verification — there's nothing live to verify yet if the file is still a skeleton). Re-running `init --api-key ... --org-id ...` afterward once the file is filled in performs the normal write-and-verify path.

## Implemented commands

| Command | Notes |
|---------|-------|
| `init [--api-key --org-id]` | Non-interactive onboarding; see "init's non-interactive design" above. Re-running bare `init` never overwrites an existing `app.json` — prints "already exists, left untouched" instead |
| `doctor` | JSON health check: `app_config` (file exists, well-formed), `api` (live `GET /v1/orgs/{org_id}` succeeds) |
| `user get --account-id <id>` | `GET https://api.atlassian.com/users/{account_id}/manage/profile` (no `/admin` prefix, no `org_id` in the path, `/profile` suffix required — see "Corrections found via live testing" below) — resolves an Atlassian `account_id` (shared across Jira/Confluence/Bitbucket since the 2019 identity unification) to email + profile, for managed accounts only. Response is wrapped: fields live under `.account.*` (e.g. `--select account.email,account.name`, not `--select email,name`) |
| `user list [--cursor <cursor>]` | `GET https://api.atlassian.com/admin/v1/orgs/{org_id}/users` (paginated via opaque `cursor` from the response's `links.next`) — every managed user in the organization in one call, each entry already including `account_id`/`name`/`email` directly (no per-user `user get` follow-up needed). Confirmed live 2026-07-22 against a real 37-person organization (single page, no `links.next` present — multi-page behavior not yet observed). Documented (not independently confirmed live) to need the `read:accounts:admin` scope specifically, distinct from `user get`'s `manage:org`/unscoped requirement — moot in practice since the configured key is already unscoped |

## Planned commands

(none currently — add rows here as concrete needs arise, per root CLAUDE.md's incremental approach)

## API design notes

- **`--select`/`--select-all`**: `--select` is mandatory by default per root `CLAUDE.md`. Exempt commands (`select.or_all()`):
  | Command | Exempt? | Why |
  |---|---|---|
  | `doctor` | yes | internally-generated report, fixed/small |
  | `user get` | yes | single profile object, fixed shape |
  | `user list` | **no** | paginated collection, size scales with org membership |
- Two API hosts/bases in play (see `endpoints.rs`'s module doc comment for the full detail): the **Organization API** (`https://api.atlassian.com/admin/v1/orgs/{org_id}`, `read:orgs:admin` scope for `doctor`'s check, `read:accounts:admin` for `user list`) used by `doctor` and `user list`; the **user management "manage" API** (`https://api.atlassian.com/users/{account_id}/manage/profile`, no `org_id` in the path at all, needs an unscoped API key) used by `user get`. `.../manage` *without* `/profile` is a different, related endpoint — a capabilities map (`apiToken.create`, `email.set`, `lifecycle.delete`, etc., each `{"allowed": bool, "reason"?: ...}`), not the profile.
- `user list`'s `cursor` query param is built via `serde_urlencoded` in `client.rs` (not string-formatted directly in `endpoints.rs` like the other path builders here), since it's an opaque, possibly base64-shaped token that could contain characters needing percent-encoding — same treatment as jira's `page_token`.
- No destructive commands — read-only API surface for now.

## Corrections found via live testing (2026-07-22)

This crate's initial implementation was written from documentation alone
(WebFetch against developer.atlassian.com proved unreliable multiple times
during design — see git history for the original, wrong assumptions) and
was **not** verified against a real organization before merging. Once real
credentials were available, `user get` failed with `404 Request failed to
match any route` — the assumed path
(`/admin/v1/orgs/{org_id}/users/{account_id}/manage`) doesn't exist. Live
`curl` testing of several path variants against the real API (see PR/session
history) found the actual route:

- **Route**: `GET https://api.atlassian.com/users/{account_id}/manage` — no
  `/admin` prefix, no `org_id` anywhere in the path. Confirmed via a `403`
  response (route matched, scope rejected) rather than a `404` (no route
  matched).
- **Scope for that route**: shows up in the `403` body's `acceptableScopes`
  as `manage:org` (also `manage/org/public-api`, `manage:me:DUMMYSCOPE`).
  **None of these appear in Atlassian's public scope catalog**
  (developer.atlassian.com/cloud/admin/scopes lists only `read:*:admin`/
  `write:*:admin`/`delete:*:admin`-shaped scopes) — meaning `manage:org`
  cannot be selected when creating a scoped ("with scopes") key at all, no
  matter how carefully the picker is searched. Confirmed live: a key created
  with **"without scopes"** (full access) reaches this route successfully;
  a scoped key holding `read:accounts:admin`/`read:directories:admin`/
  `read:orgs:admin` together still gets the same `403`.
- **The route itself wasn't the full answer either**: `GET .../manage` (no
  further suffix) returns `200` with an unscoped key, but the body is a
  *capabilities map* (`apiToken.create`, `email.set`, `lifecycle.delete`,
  etc., each `{"allowed": bool, "reason"?: {...}}`) — **not** the user's
  profile, no `email`/`name` field anywhere in it. The actual profile
  (including `email`) lives one level deeper: `GET .../manage/profile`,
  response shape `{"account": {"account_id", "name", "email", ...}}` — note
  the `account` wrapper, so `--select` needs `account.email`, not bare
  `email`. A sibling guess, `.../manage/email`, does not exist (`404`,
  plain HTML error page — a different error shape entirely from the JSON
  `404`s above, another tell that it was the wrong path).
- `read:accounts:admin`/`read:directories:admin` (this crate's originally
  selected scopes, chosen before any live verification because the names
  *sounded* plausible for "read a user's account/directory info") grant
  neither `user get`'s route nor `doctor`'s.
- `doctor`'s separate `GET /admin/v1/orgs/{org_id}` check needs its own
  `read:orgs:admin` scope — confirmed via a `403` (not `404`) when tested
  with a key holding only `read:accounts:admin`/`read:directories:admin`, and
  `200` once `read:orgs:admin` was added. Unlike `manage:org`, this scope
  *is* in the public catalog and *is* selectable — but since `user get`
  requires an unscoped key regardless, there's no reason to hunt for it
  specifically; an unscoped key satisfies both checks.

**Takeaway for future work on this crate**: don't trust a single WebFetch
against developer.atlassian.com for endpoint paths or scope names on this
API — cross-check with a live request (even a deliberately-wrong-scope one,
since a `403` vs `404` distinguishes "route exists, scope wrong" from "route
doesn't exist" without needing valid credentials yet) before writing it into
`endpoints.rs`/documentation as fact. And don't stop at the first `200` either
— check that the response body actually contains the field you're after
(`.../manage` returned `200` but the wrong shape entirely).

## Future: shared Atlassian library

Not applicable the way it is for `jira`/`bitbucket` (see "Auth design" above)
— this crate has no OAuth flow to share. If a future command needs to call
both this crate's static-key auth *and* an OAuth-authenticated Atlassian
product API in the same operation, revisit then; no such need exists yet.
