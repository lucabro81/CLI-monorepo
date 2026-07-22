# CLAUDE.md — crates/atlassian-admin

Architecture and design notes for the `atlassian-admin` crate. Global rules (TDD, error handling, flag conventions, commands) are in the root `CLAUDE.md`.

## Status

Scaffolded, not yet implemented. See "Planned commands" below.

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
cli_tests/commands split.

## Auth design

**Not OAuth — a static API key.** The Atlassian Organization Admin API
(`api.atlassian.com/admin/v1/orgs/...`) is a distinct product surface from
Jira/Confluence/Bitbucket: it authenticates with a long-lived **Organization
API key**, created by an org admin at admin.atlassian.com (Organization
settings → API keys), sent as a plain Bearer token — no OAuth grant, no
consumer, no client_id/secret pair, no expiry/refresh cycle to manage.

- **Key creation** (one-time, requires org-admin/superadmin access): admin.atlassian.com → select the organization → **Organization settings → API keys** → **Create API key** (prefer "API keys with scopes" over "without scopes"). Name it, set an expiration (max 1 year — must be manually rotated before then), select scopes, **Create**. The confirmation screen shows the **Organization ID** and the **API key together, once** — copy both immediately, they cannot be recovered afterward.
- **Scopes**: `read:accounts:admin` + `read:directories:admin` — sufficient for the `user get` command below. Prefer the narrowest scope set that covers the commands you actually use.
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

(none yet — see "Planned commands")

## Planned commands (build incrementally, smallest first)

| Command | Notes |
|---------|-------|
| `init [--api-key --org-id]` | Human onboarding; see "init's non-interactive design" above |
| `doctor` | JSON health check: `app_config` (file exists, well-formed), `api` (live `GET /v1/orgs/{org_id}` succeeds) |
| `user get --account-id <id>` | `GET /admin/v1/orgs/{org_id}/users/{account_id}/manage` — resolves an Atlassian `account_id` (shared across Jira/Confluence/Bitbucket since the 2019 identity unification) to email + profile, for managed accounts only |

## API design notes

- **`--select`/`--select-all`**: `--select` is mandatory by default per root `CLAUDE.md`. Exempt commands (`select.or_all()`):
  | Command | Exempt? | Why |
  |---|---|---|
  | `doctor` | yes | internally-generated report, fixed/small |
  | `user get` | yes | single profile object, fixed shape |
- Atlassian Organization Admin API base: `https://api.atlassian.com/admin/v1/orgs/{org_id}`.
- No pagination-shaped commands yet (both planned commands return a single object) — revisit this table if a list-returning command (e.g. `user list`) is ever added.
- No destructive commands — read-only API surface for now.

## Future: shared Atlassian library

Not applicable the way it is for `jira`/`bitbucket` (see "Auth design" above)
— this crate has no OAuth flow to share. If a future command needs to call
both this crate's static-key auth *and* an OAuth-authenticated Atlassian
product API in the same operation, revisit then; no such need exists yet.
