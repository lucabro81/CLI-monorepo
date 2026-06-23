# CLAUDE.md — crates/google-chat

Architecture and design notes for the `google-chat` crate. Global rules (TDD, error handling, flag conventions, commands) are in the root `CLAUDE.md`.

## Module map

```
src/
  commands/
    mod.rs        — pub mod declarations for all command handlers
    auth.rs       — run_login()
    doctor.rs     — run_doctor(); also called by init as final verification
    init.rs       — run_init(), write_app_config(); human onboarding flow
    spaces.rs     — run(SpacesCommand); dispatches space subcommands
    messages.rs   — run(MessagesCommand); dispatches message subcommands
  auth.rs         — OAuth infrastructure: OAuthConfig, Credentials, login(),
                    refresh(), renew(), save_credentials(), load_credentials(),
                    path helpers
  client.rs       — GoogleChatClient (blocking reqwest); get_json/post_json
                    helpers; all Chat API methods: list_spaces, list_messages,
                    create_message
  cli.rs          — clap structs: Cli (--select global), Command, AuthCommand,
                    SpacesCommand, MessagesCommand. No logic.
  context.rs      — config_dir(), load_oauth_config(), authenticated_client(),
                    print_json(value, select). Shared by all command handlers.
  endpoints.rs     — URL/path constants for Google OAuth and the Chat API v1,
                    used by auth.rs and client.rs. No logic.
  error.rs        — CliError (top-level, thiserror-derived). Includes IoError
                    for stdin prompts in init.
  fields.rs       — filter_fields(value, select): dot-notation projection,
                    array-aware, backed by FieldTree (recursive BTreeMap).
  tests/          — all *_tests.rs files, mirroring the src/ layout (see root
                    CLAUDE.md's "Test file convention").
  main.rs         — pure dispatch: parse --select, match Command, call commands::*.
```

## OAuth / auth design

Two grant types, mirroring jira's pattern but with Google-specific mechanics:

- **Service account + domain-wide delegation** (default, `auth login`) —
  `login_service_account()` signs a JWT assertion (RFC 7523) with the service
  account's private key from `app.json`'s `service_account` block, with
  `sub` set to the impersonated Workspace user (the dedicated "service user"
  account for this automation), and exchanges it at the token endpoint via
  `grant_type=urn:ietf:params:oauth:grant-type:jwt-bearer`. No browser, no
  user interaction. Returns `Credentials` with `refresh_token: None` —
  renewal re-signs and re-exchanges a fresh assertion rather than refreshing.
  This is the expected mode for agent-driven usage.
- **Authorization Code + PKCE** (`auth login --user`) — interactive consent
  flow for a human Google account: PKCE challenge generation, browser
  launch, one-shot local HTTP server for the callback, authorization code
  exchange. Returns `Credentials` with `refresh_token: Some(...)`.

Both grants request the same scopes and call the same Chat API surface —
unlike jira there's no separate `audience`/cloud-id concept, since Google
Chat has no tenant-resolution step: API calls are scoped directly by space
resource name (`spaces/{space}`) under whichever identity is authenticated
(impersonated service user, or the human who logged in).

**Service account flow setup** (one-time, requires Workspace super-admin):
1. Create a service account in Google Cloud Console (IAM & Admin → Service
   Accounts), download its JSON key.
2. Enable "Google Workspace Domain-wide Delegation" on that service account;
   note its numeric OAuth Client ID.
3. In Google Admin Console (Security → Access and data control → API
   controls → Domain-wide delegation), authorize that Client ID for exactly
   this CLI's scopes (see below).
4. Add a `service_account` block to `app.json` (see Config layout) with
   `client_email`/`private_key` from the downloaded key and
   `impersonate_user` set to the service user's email.

**3LO flow**: the human-login path requires no extra Google Cloud setup
beyond the OAuth client (`client_id`/`client_secret`) already in `app.json`
for this purpose; just run `auth login --user` and approve the consent
screen.

**Renewal**: before each API call, the CLI checks whether the access token
is expired (or about to expire within 60s). Credentials with a
`refresh_token` (3LO) are renewed via the `refresh_token` grant; credentials
with none (service account) are renewed by re-running
`login_service_account` with a freshly signed JWT. Unlike Atlassian, Google
refresh tokens for an **Internal** consent-screen app don't rotate or expire
on a fixed schedule — no rotate-on-every-use concern for the 3LO path.

**Scopes** (both grants): `https://www.googleapis.com/auth/chat.spaces.readonly
https://www.googleapis.com/auth/chat.messages.readonly
https://www.googleapis.com/auth/chat.messages.create`.

**Token endpoint requests must be `application/x-www-form-urlencoded`**, not
JSON — Google's `jwt-bearer` grant rejects a JSON body with
`unsupported_grant_type`. All three grants (`authorization_code`,
`refresh_token`, `jwt-bearer`) use the same form-encoded `request_token`
helper for consistency.

**Consent screen type matters** for the 3LO path: the OAuth client must be
configured as **Internal** (Workspace-restricted) in Google Cloud Console.
External + Testing apps get refresh tokens that expire after 7 days,
requiring frequent re-login — unacceptable for a long-lived CLI. Internal
also skips Google's verification process for these scopes entirely.

## Config layout (XDG-style)

Both files live under `$XDG_CONFIG_HOME/google-chat-cli/` (falling back to
`~/.config/google-chat-cli/`):

- `app.json` — static; written by `google-chat init` or by hand. Never
  modified at runtime.
  ```json
  {
    "client_id": "...",
    "client_secret": "...",
    "service_account": {
      "client_email": "...",
      "private_key": "...",
      "impersonate_user": "service-user@example.com"
    }
  }
  ```
  `service_account` is optional — required only for the default
  (`auth login`, no `--user`) flow; omit it if only the interactive 3LO
  flow will ever be used.
- `credentials.json` — OAuth tokens. Fully managed by the CLI; never edit by
  hand.

## API design notes

- **Pagination**: Chat API list endpoints (`spaces.list`,
  `spaces.messages.list`) use `pageSize` + `pageToken` (opaque cursor from
  the previous response's `nextPageToken`), same shape as jira's
  `nextPageToken` cursor.
- **`messages list` as context recovery**: this command is the primary way
  an agent re-establishes conversation context after a gap or aggressive
  history summarization — it must support paging back through a space's
  history (not just "latest N"), so default ordering is chronological with
  full pagination support, not a fixed-size tail.
- **No `auth whoami`**: confirmed there is no Chat API endpoint that
  identifies the authenticated human user with the scopes this CLI requests
  (`chat.spaces.readonly`/`chat.messages.readonly`/`chat.messages.create`).
  `users/app` is an alias for the calling bot identity, not the OAuth user;
  getting real identity data would require additional `openid`/`profile`/
  `email` scopes (or the separate People API), which was explicitly ruled
  out. `doctor`'s `api` check (a live `spaces.list` call) is the
  auth-sanity-check instead of a dedicated whoami command.
- **`--select`** (global flag): client-side dot-notation projection via
  `fields::filter_fields`. Applied by `context::print_json` before printing.

## Implemented commands

| Command | Notes |
|---------|-------|
| `auth login [--user]` | Default: domain-wide-delegation (service account, no browser). `--user`: interactive OAuth 2.0 + PKCE |

## Planned commands

| Command | Notes |
|---------|-------|
| `init [--client-id --client-secret]` | Human onboarding; only command with narrative output |
| `doctor` | Cascading JSON health check (app_config, credentials, api); exit non-zero on any failure |
| `spaces list` | Lists spaces (`spaces.list`) the authenticated user belongs to — id, displayName, type. Paginated. |
| `messages list --space <id>` | Lists messages in a space (`spaces.messages.list`), paginated, chronological. Doubles as the context-recovery path for an agent resuming after a gap or summarization. |
| `messages send --space <id> --text <text>` | Creates a message (`spaces.messages.create`) in a space. |

## Known edge cases (see BACKLOG.md)

None yet — this crate is newly scaffolded. Use prefix `GCHAT-` for entries
added as commands are implemented.
