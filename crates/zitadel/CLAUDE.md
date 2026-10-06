# CLAUDE.md — crates/zitadel

Architecture and design notes for the `zitadel` crate. Global rules (TDD, error handling, flag conventions, commands) are in the root `CLAUDE.md`.

## Status

`init [--user]`, `doctor [--user]`, `auth login [--user]`, `auth whoami [--user]`, `user search`, `user get`, `organization list`, `project list` implemented (issue #142). New commands are added one at a time via the `add-cli-command` skill.

## Module map (mirrors crates/google-chat)

```
src/
  commands/
    mod.rs           — pub mod declarations for all command handlers
    auth.rs          — run_login(), run_whoami()                         [implemented]
    doctor.rs        — run_doctor(identity)/run_doctor_in(dir, identity),
                       summarize_memberships(), check_identities(); also
                       called by init as final check                    [implemented]
    init.rs          — run_init(), build_app_config() (merge flags over existing
                       app.json), write_app_config() (mode 0600)         [implemented]
    user.rs          — run(UserCommand); build_search_body() (pure)    [search, get implemented]
    organization.rs  — run(OrganizationCommand); build_list_body() (pure) [list implemented]
    project.rs       — run(ProjectCommand); build_list_body() (pure)    [list implemented]
  auth.rs         — AppConfig, ServiceUserKey (+ from_key_file: validates type/PEM),
                    Credentials; JWT-profile login
                    (service user), authorization code + PKCE login (--user),
                    renew(), load_credentials()/save_credentials(),
                    credentials_path(dir, identity), legacy_credentials_path(); PKCE,
                    the callback listener and Identity come from crates/oauth-user-login
  client.rs       — ZitadelClient (blocking reqwest); get_json/post_json helpers; url_with_segment()
                    percent-encodes ids as one path segment; ClientError::{Request, Status}
                    [get_current_user, list_my_memberships, search_users, get_user, list_organizations,
                    list_projects implemented]
  cli.rs          — clap structs only, no logic
  context.rs      — config_dir(), load_app_config(), authenticated_client(identity),
                    login_error_to_cli(e, path, identity), login_command(identity),
                    client_error_to_cli() (401 → re-login hint, 403 → missing-role hint
                    pointing to doctor, 404 → verify-id hint), print_json(value, select),
                    search_query(limit, offset) + CONTAINS_IGNORE_CASE (shared by
                    every v2 search body)
  endpoints.rs    — path constants/builders relative to the instance URL, no logic
  error.rs        — CliError (thiserror), incl. transparent Select(cli_fields::RenderError)
  tests/          — *_tests.rs mirroring src/; auth_user_tests.rs = the --user flow
                    (authorize URL, callback error wrapping, exchange, refresh, renew dispatch); (root CLAUDE.md "Test file convention");
                    test_support.rs = one-shot / sequential local HTTP mock servers shared by
                    auth/client tests; fixtures/ = throwaway RSA key pair (test-only)
  main.rs         — pure dispatch: resolve --select/--select-all and --user (Identity)
                    once, call commands::*
```

## Auth design

Zitadel is its own identity platform (Cloud `https://<instance>.zitadel.cloud` or
self-hosted on a custom domain) — every URL is relative to the configured
`instance_url`. Not Atlassian, so `crates/atlassian-auth` does not apply. PKCE,
`state` and the loopback callback listener come from the shared
`crates/oauth-user-login` library (root CLAUDE.md).

Two identities, stored side by side (issue #164, root `CLAUDE.md`'s "Two identities
per CLI"): the global `--user` flag selects, per call, the service user (default,
`credentials-service.json`) or the human (`credentials-user.json`);
`context::authenticated_client(identity)` loads and renews only that one, and
`LoginMode::identity()` decides which file a login writes. `app.json` already held
both parts (`service_user` key, Native app `client_id`), so its shape is unchanged.
`--remote`/`--code` without `--user` fail in `LoginMode::from_flags`
(`CliError::RemoteLoginNeedsUser`), since clap can't see a global `--user` placed
before the subcommand. `init --user` logs in as the human through the browser; a
changed instance URL removes both credentials files.

- **`auth login` (default) — service user, private key JWT** (Zitadel's
  recommended service-account method; no human step, intended for agents).
  - Key file downloaded from the console:
    `{"type":"serviceaccount","keyId":"...","key":"-----BEGIN RSA PRIVATE KEY-----...","userId":"..."}`.
    `init --key-file <path>` copies it inline into `app.json` under `service_user`.
  - Assertion: header `alg=RS256`, `kid=keyId`; claims `iss=sub=userId`,
    `aud=instance_url`, `iat=now`, `exp=now+3600`.
  - `POST <instance_url>/oauth/v2/token` (form):
    `grant_type=urn:ietf:params:oauth:grant-type:jwt-bearer`, `assertion`,
    `scope=openid urn:zitadel:iam:org:project:id:zitadel:aud` (the audience scope is
    what lets the token call Zitadel's own APIs).
  - No refresh token: renewal re-signs a fresh assertion.
- **`auth login --user` — authorization code + PKCE (S256)**, for a human.
  - Needs a **Native** application in a Zitadel project (public client, auth
    method PKCE, redirect `http://localhost:8080/callback`, development mode on
    because of `http`, refresh token enabled). Its Client ID goes in `app.json`.
  - `<instance_url>/oauth/v2/authorize` → browser → callback on `127.0.0.1:8080`
    (`state` checked; stray requests like `/favicon.ico` get 404 and the listener
    keeps waiting) → `/oauth/v2/token` with `code_verifier`.
  - Scopes: `openid profile email offline_access urn:zitadel:iam:org:project:id:zitadel:aud`.
  - Refresh via `refresh_token` grant; a response without a new refresh token keeps
    the old one (a user session never silently turns into the service user).
- **`auth login --user --remote` — the same grant in two steps**, for a person not at
  this machine (issue #146). Step 1 (`start_remote_login`) saves an
  `oauth_user_login::PendingLogin` to `pending-login.json` (0600, 10 minutes) and
  prints `{authorize_url, state, expires_at}`; step 2 (`complete_remote_login`, `--code
  --state`) takes it (state single-use, consumed before the exchange), exchanges the
  code with the stored verifier and redirect URI, saves `credentials-user.json` and
  prints `auth whoami --user`. The remote redirect URI must be registered on the Native app next
  to the loopback one. `doctor` reports it as `pending_login` (outside `all_ok`).
- **Renewal**: `load_credentials` renews when `now + 60 >= expires_at` —
  `refresh_token` present → refresh grant, else re-sign the JWT.
- **Authorization** is Zitadel's manager roles on the identity (IAM_OWNER,
  ORG_OWNER, ORG_USER_MANAGER, PROJECT_OWNER, …), not OAuth scopes. `doctor`
  reports them so an LLM can tell what it is allowed to do.

## Config layout

`$XDG_CONFIG_HOME/zitadel-cli/` (fallback `~/.config/zitadel-cli/`):

- `app.json` — `{"instance_url": "...", "service_user": {"keyId","key","userId"}?, "client_id": "..."?}`.
  Written by `init`; `service_user` is needed for the default login, `client_id` for `--user`.
- `credentials-service.json` / `credentials-user.json` — `access_token`, `expires_at`,
  `refresh_token?` (only the human's). Fully CLI-managed. A leftover pre-#164
  `credentials.json` is not read; `doctor`'s `identities.legacy_credentials_file` reports it.
- `pending-login.json` — only between the two steps of `auth login --user --remote`:
  `state`, `code_verifier`, `redirect_uri`, `expires_at`. Removed by step 2.

All of them hold secrets and are written mode `0600` (an existing looser file loses its group/other bits).

## API design notes

- Zitadel v2 REST JSON under `<instance_url>/v2/...` (recommended by Zitadel for new
  integrations); v1 (`/management/v1`, `/auth/v1`) only where v2 has no equivalent.
- Searches are `POST` with a JSON body: `{"query": {"offset", "limit", "asc"}, "queries": [...]}`.
  Pagination is offset/limit; the raw response (incl. `details.totalResult`) is passed through.
  Top-level `queries` are combined with AND. With zero matches the response has no `result`
  and no `totalResult` (only `details.timestamp`).
- **Some v2 services have no REST mapping.** The v2 `ProjectService` returns 404 on any
  `/v2/projects...` REST path; it is reachable only via its Connect-protocol path
  (`POST /zitadel.project.v2.ProjectService/<Method>`) with a plain `application/json`
  body (`application/connect+json` → 415). These newer services also use a different
  shape: request `{"pagination": {...}, "filters": [...]}` with `TEXT_FILTER_METHOD_*`,
  response `{"pagination": {"totalResult", "appliedLimit"}, "projects": [...]}`. Probe a
  new service with both forms before assuming either.
- **Enum filters must be validated client-side** (clap `ValueEnum`): ZITADEL answers an unknown
  enum value (e.g. `"state": "BOGUS"`) with 200 and zero results, not an error — an LLM would
  read that as "nothing matches".

## Implemented commands

| Command | Endpoint | `--select` |
|---|---|---|
| `auth login` | `POST /oauth/v2/token` (jwt-bearer) | n/a (prints a confirmation line) |
| `auth login --user` | `GET /oauth/v2/authorize` (browser) + `POST /oauth/v2/token` (authorization_code + PKCE, refresh_token) | n/a |
| `auth login --user --remote` / `--code --state` | step 1 prints the authorize URL (no request); step 2 `POST /oauth/v2/token` (authorization_code + PKCE) then `GET /auth/v1/users/me` | exempt (`or_all`): step 1's synthesized object, step 2 = whoami |
| `init [--user]` | writes app.json, logs in the selected identity (`POST /oauth/v2/token`; browser with `--user`), runs doctor for it; flags only, no prompts; narrative on stderr, doctor report on stdout | exempt (`or_all`), like doctor |
| `doctor [--user]` | checks the selected identity (informational `pending_login` and `identities`): `GET /auth/v1/users/me` + `POST /auth/v1/memberships/me/_search` (v1: no v2 equivalent for the caller's own roles) | exempt (`or_all`) |
| `user search` | `POST /v2/users` (v2 `ListUsers`) | mandatory |
| `user get <user-id>` | `GET /v2/users/{userId}` (v2 `GetUserByID`) | exempt (`or_all`) |
| `organization list` | `POST /v2/organizations/_search` (v2 `ListOrganizations`); visibility follows roles (`IAM_OWNER` all, `ORG_OWNER` own only) | mandatory |
| `project list` | `POST /zitadel.project.v2.ProjectService/ListProjects` (Connect path, see API notes) | mandatory |
| `auth whoami` | `GET /auth/v1/users/me` (v1: no v2 "me" endpoint; `/oidc/v1/userinfo` only returns `sub` with the `openid` scope) | exempt (`or_all`) |

## Testing

```sh
cargo test -p zitadel                 # unit tests, no credentials
cargo test -p zitadel -- --ignored    # e2e against a real instance (see ADDENDUM)
```

`tests/e2e_tests.rs` is read-only and self-referential: the logged-in identity is the fixture.
