# CLAUDE.md — crates/google-chat

Architecture and design notes for the `google-chat` crate. Global rules (TDD, error handling, flag conventions, commands) are in the root `CLAUDE.md`.

## Module map

```
src/
  commands/
    mod.rs        — pub mod declarations for all command handlers
    auth.rs       — run_login(), run_whoami()
    doctor.rs     — run_doctor(); also called by init as final verification
    init.rs       — run_init(), write_app_config(); human onboarding flow
    spaces.rs     — run(SpacesCommand); dispatches space subcommands
    messages.rs   — run(MessagesCommand); dispatches message subcommands
  auth.rs         — OAuth infrastructure: OAuthConfig, Credentials, login(),
                    refresh(), renew(), save_credentials(), load_credentials(),
                    path helpers
  client.rs       — GoogleChatClient (blocking reqwest); get_json/post_json
                    helpers; all Chat API methods: list_spaces, list_messages,
                    create_message, get_myself
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

Single grant type: **OAuth 2.0 Authorization Code + PKCE** (installed-app
flow), used by both `auth login` and `init`. Unlike jira/bitbucket, there is
no service-account / `client_credentials` equivalent for acting *as a user*
in Google Chat — that grant is for bot/app identities, not for reading or
sending messages as yourself — so `auth login` has no `--user` flag branch.

1. **Authorization request** — the CLI generates a PKCE `code_verifier` and
   `code_challenge` (SHA-256 + base64url) and a random `state`, builds the
   authorization URL against `https://accounts.google.com/o/oauth2/v2/auth`
   with the requested scopes and `redirect_uri=http://localhost:8080/callback`,
   and opens it in the browser.
2. **Local callback** — a one-shot TCP listener on `127.0.0.1:8080` receives
   the redirect, verifies `state`, and replies with a small HTML confirmation
   page.
3. **Token exchange** — POSTs the authorization `code`, PKCE `code_verifier`,
   and `client_id`/`client_secret` to `https://oauth2.googleapis.com/token`,
   receiving `access_token`, `refresh_token`, and expiry.
4. **No tenant resolution step** — unlike jira's `cloud_id`, Google Chat API
   calls are scoped directly by space resource name (`spaces/{space}`) under
   the authenticated user; nothing to resolve after token exchange.
5. **Persisting credentials** — `access_token`, `refresh_token`, `expires_at`
   written to `credentials.json`.

**Renewal**: before each API call, the CLI checks whether the access token
is expired (or about to expire within 60s) and exchanges `refresh_token` for
a new access token via the same token endpoint. Unlike Atlassian, Google
refresh tokens for an **Internal** consent-screen app don't rotate or expire
on a fixed schedule — no need to treat the refresh token itself as
single-use.

**Scopes**: `https://www.googleapis.com/auth/chat.spaces.readonly
https://www.googleapis.com/auth/chat.messages.readonly
https://www.googleapis.com/auth/chat.messages.create`.

**Consent screen type matters**: this app must be configured as **Internal**
(Workspace-restricted) in Google Cloud Console. External + Testing apps get
refresh tokens that expire after 7 days, requiring frequent re-login —
unacceptable for a long-lived CLI. Internal also skips Google's verification
process for these scopes entirely.

## Config layout (XDG-style)

Both files live under `$XDG_CONFIG_HOME/google-chat-cli/` (falling back to
`~/.config/google-chat-cli/`):

- `app.json` — `{"client_id": "...", "client_secret": "..."}`. Static;
  written by `google-chat init` or by hand from a downloaded OAuth client
  credentials JSON. Never modified at runtime.
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
- **`--select`** (global flag): client-side dot-notation projection via
  `fields::filter_fields`. Applied by `context::print_json` before printing.

## Implemented commands

| Command | Notes |
|---------|-------|
| (none yet — bootstrap in progress) | |

## Planned commands

| Command | Notes |
|---------|-------|
| `init [--client-id --client-secret]` | Human onboarding; only command with narrative output |
| `doctor` | Cascading JSON health check (app_config, credentials, api); exit non-zero on any failure |
| `auth login` | Interactive OAuth 2.0 (3LO) + PKCE flow (only grant type) |
| `auth whoami` | Identifies the authenticated user |
| `spaces list` | Lists spaces (`spaces.list`) the authenticated user belongs to — id, displayName, type. Paginated. |
| `messages list --space <id>` | Lists messages in a space (`spaces.messages.list`), paginated, chronological. Doubles as the context-recovery path for an agent resuming after a gap or summarization. |
| `messages send --space <id> --text <text>` | Creates a message (`spaces.messages.create`) in a space. |

## Known edge cases (see BACKLOG.md)

None yet — this crate is newly scaffolded. Use prefix `GCHAT-` for entries
added as commands are implemented.
