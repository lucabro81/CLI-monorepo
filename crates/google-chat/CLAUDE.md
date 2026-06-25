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
    subscription.rs — run(SubscriptionCommand); dispatches to EventsClient
    listen.rs     — run_listen(); the crate's one async command (see below)
  auth.rs         — OAuth infrastructure: OAuthConfig, Credentials, login(),
                    refresh(), renew(), save_credentials(), load_credentials(),
                    path helpers
  client.rs       — GoogleChatClient (blocking reqwest); get_json/post_json
                    helpers; all Chat API methods: list_spaces, list_messages,
                    create_message; normalize_space_name (pub(crate), reused
                    by events_client.rs)
  events_client.rs — EventsClient (blocking reqwest) for the Workspace Events
                    API and the Pub/Sub admin API: ensure_pubsub_subscription
                    (idempotent — 409 ALREADY_EXISTS treated as success),
                    create_workspace_events_subscription,
                    renew_subscription (PATCH ttl=0s, resets TTL to max —
                    used by listen.rs's background renewal task),
                    delete_subscription (DELETE, stops delivery immediately).
                    Same bearer token as GoogleChatClient, different base
                    URLs/scopes. EventsClientError::into_pubsub_error /
                    into_workspace_events_error map to CliError, shared by
                    commands/subscription.rs and commands/listen.rs.
  cli.rs          — clap structs: Cli (--select global), Command, AuthCommand,
                    SpacesCommand, MessagesCommand, SubscriptionCommand. No logic.
  context.rs      — config_dir(), load_oauth_config(), authenticated_credentials(),
                    authenticated_client(), print_json(value, select). Shared
                    by all command handlers; authenticated_credentials() is
                    also used directly by events_client/listen callers that
                    need the raw access token rather than a GoogleChatClient.
  endpoints.rs     — URL/path constants for Google OAuth, the Chat API v1, the
                    Workspace Events API v1, and the Pub/Sub API v1, used by
                    auth.rs, client.rs, and events_client.rs. No logic.
  error.rs        — CliError (top-level, thiserror-derived). Includes IoError
                    for stdin prompts in init, and WorkspaceEvents*/Pubsub*
                    variants for the new commands.
  fields.rs       — filter_fields(value, select): dot-notation projection,
                    array-aware, backed by FieldTree (recursive BTreeMap).
  tests/          — all *_tests.rs files, mirroring the src/ layout (see root
                    CLAUDE.md's "Test file convention").
  main.rs         — pure dispatch: parse --select, match Command, call commands::*.
```

## Async: `listen` only

Every command except `listen` is synchronous (`reqwest::blocking`) — this
crate deliberately avoids a project-wide async runtime. `listen` needs
`google-cloud-pubsub`'s streaming pull, which is tokio-async only; rather
than convert the whole crate, `commands::listen::run_listen` builds its own
`tokio::runtime::Runtime` and tears it down when the command returns. No
other module is aware of tokio.

To let the Pub/Sub subscriber reuse this crate's own OAuth access token
(instead of Application Default Credentials), `listen.rs` implements
`google_cloud_auth::credentials::CredentialsProvider` on a small
`SharedTokenCredentials` adapter wrapping `Arc<RwLock<String>>`. A background
task polls `context::authenticated_credentials()` (which only actually
renews when within 60s of expiry) every 5 minutes via `spawn_blocking` and
writes the refreshed token into that shared state — this is what lets
`listen` run past the ~1h access-token lifetime without being restarted.

A second background task renews the Workspace Events subscription itself
every 30 minutes (`EventsClient::renew_subscription`, also via
`spawn_blocking`, reading the same shared token) — that subscription has its
own ~4h TTL on Google's side, independent of the OAuth access token, and
`listen` needs the subscription's `name` (`--workspace-events-subscription`)
to keep it alive. Both background tasks and the pull loop race in the same
`tokio::select!`.

Shutdown is handled by racing the pull loop against both `SIGINT` (Ctrl+C)
and `SIGTERM` (`kill`/`pkill` — the way an agent or script controlling the
process as a background job would normally stop it); the PID is logged to
stderr at startup so the caller has something to send the signal to.

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

**Status: not yet activated** (see `BACKLOG.md` GCHAT-1). The code above is
implemented and unit-tested, but step 2-3 need a Workspace super-admin, not
available right now. This is planned, not abandoned — it'll be turned on as
soon as that access is available, with no code changes needed. Until then,
day-to-day usage runs on `auth login --user` (3LO, below), logged in as the
operator's own Google account.

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
https://www.googleapis.com/auth/chat.messages.create
https://www.googleapis.com/auth/chat.memberships.readonly
https://www.googleapis.com/auth/pubsub`. The last two were added for
`subscription create`/`listen` — verified live (`BACKLOG.md` GCHAT-3):
`chat.spaces.readonly` + `chat.memberships.readonly` are sufficient for
Workspace Events subscriptions, no extra scope needed.

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
- **Space identifier normalization**: `--space` flags accept either the bare
  space id or the full `spaces/{id}` resource name (`client::normalize_space_name`),
  so a caller can paste either form straight from `spaces list`'s `name` field.
  Used by both `messages list` and `messages send`.
- **`messages send` is not `--confirm`-gated**: unlike jira's `issue delete`,
  sending a message isn't irreversible data destruction — it's visible,
  ordinary chat activity. No confirmation flag.
- **`post_json`**: `client.rs` gained a `post_json` helper (mirroring jira's)
  for `create_message` — the crate's first write call. Same
  bearer-auth/status-check/JSON-decode shape as `get_json`.
- **`subscription create --space spaces/-` (the "all spaces" wildcard
  `targetResource`) is intentionally not the recommended/documented usage**:
  it grants visibility into every space the authenticated identity belongs
  to, not just the ones an agent is actively engaged in — a much broader
  blast radius than needed, and the wrong default for an agent that should
  only see the conversation it's actually part of. The intended pattern is
  one `subscription create --space <id>` per conversation the agent is
  currently in, all sharing the same Pub/Sub topic/subscription/`listen`
  process if convenient (the topic is just transport — it doesn't broaden
  access; only an explicit Workspace Events subscription does), paired with
  `subscription delete --name <name>` when the agent is done with that
  conversation, rather than relying solely on the ~4h natural expiry.

## Implemented commands

| Command | Notes |
|---------|-------|
| `auth login [--user]` | Default: domain-wide-delegation (service account, no browser) — implemented, not yet verified live (GCHAT-1). `--user`: interactive OAuth 2.0 + PKCE — verified live, current day-to-day path |
| `doctor` | Cascading JSON health check (app_config, credentials, api); exit non-zero on any failure. Verified live against a real Workspace via the `--user` flow. |
| `init [--client-id --client-secret]` | Human onboarding; only command with narrative output. `write_app_config` preserves an existing `service_account` block across reruns. |
| `spaces list [--page-size --page-token]` | Lists spaces (`spaces.list`) the authenticated identity belongs to. Verified live — real spaces returned, types (`SPACE`/`GROUP_CHAT`/`DIRECT_MESSAGE`) confirmed. |
| `messages list --space <id> [--page-size --page-token --order-by]` | Lists messages in a space (`spaces.messages.list`). Chronological by default (`createTime ASC`, the Chat API's own default) — the context-recovery path for an agent resuming after a gap or summarization. `--order-by "createTime DESC"` gets the most recent first. `--space` accepts bare id or full `spaces/{id}`. Verified live against real conversation history both orderings, both id forms. |
| `messages send --space <id> --text <text>` | Creates a message (`spaces.messages.create`) in a space; prints the created Message (including its `name`). Not gated by `--confirm` — visible but not data-destructive. Verified live: real message delivered and visible to the other party, both `--space` id forms confirmed. |
| `subscription create --space <id> --topic <topic> --pubsub-subscription <sub> [--event-type ...]` | Ensures the Pub/Sub pull subscription exists (idempotent), then creates a Workspace Events subscription delivering Chat events for the space to that topic. Verified live — real subscription created, `state: ACTIVE`. The subscription expires after ~4h (Workspace Events API's own default TTL); pass its `name` to `listen --workspace-events-subscription` to keep it renewed (BACKLOG.md GCHAT-4). |
| `subscription delete --name <name>` | Deletes a Workspace Events subscription, stopping delivery immediately — call when an agent leaves a conversation, instead of relying on the ~4h expiry. Verified live: real subscription deleted (`done: true`); a second delete on the same name correctly returned `403 SUBSCRIPTION_ACCESS_DENIED` (Workspace Events conflates "gone" with "no permission" in this error). |
| `listen --pubsub-subscription <sub> --workspace-events-subscription <name> [--max-messages N]` | Streams messages from a Pub/Sub subscription via `google-cloud-pubsub`, printing each as NDJSON and acking it. Refreshes its own token every 5 min and renews the Workspace Events subscription's TTL every 30 min, both in the background; stops cleanly on SIGINT/SIGTERM. Verified live — a real `messages send` was received and printed within ~2s, `kill -TERM <pid>` exited cleanly, and the renewal PATCH call was confirmed directly (pushed `expireTime` out another ~4h, same scopes, no extra scope needed). Neither background task's periodic *trigger* was observed firing during an actual `listen` run (would need a 5/30-minute-long session) — the calls they invoke were verified directly instead (BACKLOG.md GCHAT-3, GCHAT-4). |

## Planned commands

(none — new commands land as concrete needs arise, per root CLAUDE.md's incremental approach)

## Known edge cases (see BACKLOG.md)

See `BACKLOG.md` GCHAT-1 through GCHAT-3. Use prefix `GCHAT-` for new entries
as commands are implemented.
