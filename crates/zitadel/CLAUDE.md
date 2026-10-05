# CLAUDE.md — crates/zitadel

Architecture and design notes for the `zitadel` crate. Global rules (TDD, error handling, flag conventions, commands) are in the root `CLAUDE.md`.

## Status

`auth login` (service user), `auth whoami` implemented. Rest in progress — see "Planned commands" below and tracking issue #142.

## Module map (mirrors crates/google-chat)

```
src/
  commands/
    mod.rs           — pub mod declarations for all command handlers
    auth.rs          — run_login(), run_whoami()                         [implemented, --user planned]
    doctor.rs        — run_doctor(); also called by init as final check  [planned]
    init.rs          — run_init(), write_app_config(); onboarding flow   [planned]
    user.rs          — run(UserCommand): search, get                     [planned]
    organization.rs  — run(OrganizationCommand): list                    [planned]
    project.rs       — run(ProjectCommand): list                         [planned]
  auth.rs         — AppConfig, ServiceUserKey, Credentials; JWT-profile login
                    (service user), authorization code + PKCE login (--user),
                    renew(), load_credentials()/save_credentials(), callback parsing
  client.rs       — ZitadelClient (blocking reqwest); get_json helper; ClientError::{Request, Status}
                    [get_current_user implemented]
  cli.rs          — clap structs only, no logic
  context.rs      — config_dir(), load_app_config(), authenticated_client(),
                    client_error_to_cli() (401 → re-login hint, 403 → missing-role hint
                    pointing to doctor), print_json(value, select)
  endpoints.rs    — path constants/builders relative to the instance URL, no logic
  error.rs        — CliError (thiserror), incl. transparent Select(cli_fields::RenderError)
  tests/          — *_tests.rs mirroring src/ (root CLAUDE.md "Test file convention");
                    test_support.rs = one-shot local HTTP mock server shared by
                    auth/client tests; fixtures/ = throwaway RSA key pair (test-only)
  main.rs         — pure dispatch: resolve --select/--select-all once, call commands::*
```

## Auth design

Zitadel is its own identity platform (Cloud `https://<instance>.zitadel.cloud` or
self-hosted on a custom domain) — every URL is relative to the configured
`instance_url`. Not Atlassian, so `crates/atlassian-auth` does not apply. The
loopback/PKCE helpers are copy-adapted from `google-chat` (see issue #143 for the
shared-library evaluation).

Two identities, same `credentials.json` — the last login decides (bitbucket model):

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
- **Renewal**: `load_credentials` renews when `now + 60 >= expires_at` —
  `refresh_token` present → refresh grant, else re-sign the JWT.
- **Authorization** is Zitadel's manager roles on the identity (IAM_OWNER,
  ORG_OWNER, ORG_USER_MANAGER, PROJECT_OWNER, …), not OAuth scopes. `doctor`
  reports them so an LLM can tell what it is allowed to do.

## Config layout

`$XDG_CONFIG_HOME/zitadel-cli/` (fallback `~/.config/zitadel-cli/`):

- `app.json` — `{"instance_url": "...", "service_user": {"keyId","key","userId"}?, "client_id": "..."?}`.
  Written by `init`; `service_user` is needed for the default login, `client_id` for `--user`.
- `credentials.json` — `access_token`, `expires_at`, `refresh_token?`. Fully CLI-managed.

## API design notes

- Zitadel v2 REST JSON under `<instance_url>/v2/...` (recommended by Zitadel for new
  integrations); v1 (`/management/v1`, `/auth/v1`) only where v2 has no equivalent.
- Searches are `POST` with a JSON body: `{"query": {"offset", "limit", "asc"}, "queries": [...]}`.
  Pagination is offset/limit; the raw response (incl. `details.totalResult`) is passed through.

## Implemented commands

| Command | Endpoint | `--select` |
|---|---|---|
| `auth login` | `POST /oauth/v2/token` (jwt-bearer) | n/a (prints a confirmation line) |
| `auth whoami` | `GET /auth/v1/users/me` (v1: no v2 "me" endpoint; `/oidc/v1/userinfo` only returns `sub` with the `openid` scope) | exempt (`or_all`) |

## Planned commands (issue #142)

| Command | Endpoint (verify in add-cli-command step 3) | `--select` |
|---|---|---|
| `init` | writes app.json, logs in, runs doctor | narrative output |
| `doctor` | userinfo + memberships | exempt (`or_all`) |
| `auth login --user` | `/oauth/v2/authorize` + `/oauth/v2/token` | n/a |
| `user search` | `POST /v2/users` | mandatory |
| `user get <user-id>` | `GET /v2/users/{userId}` | exempt |
| `organization list` | `POST /v2/organizations/_search` | mandatory |
| `project list` | v2 ProjectService ListProjects (fallback v1 `/management/v1/projects/_search`) | mandatory |

## Testing

```sh
cargo test -p zitadel                 # unit tests, no credentials
cargo test -p zitadel -- --ignored    # e2e against a real instance (see ADDENDUM)
```
