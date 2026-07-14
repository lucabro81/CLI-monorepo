# google-chat

CLI for Google Chat (Google Workspace), designed to be driven by an LLM agent (output is JSON, errors are actionable). This README documents it for humans setting it up and maintaining it; new commands get documented here as they're added.

## Table of contents

- [Setup](#setup)
- [How the OAuth flow works](#how-the-oauth-flow-works)
- [Usage](#usage)
- [Testing](#testing)
- [Error design](#error-design)

## Setup

### 1. Google Cloud project with the Chat API enabled

Create (or reuse) a Google Cloud project and enable the **Google Chat API**
for it, from [console.cloud.google.com/apis/library](https://console.cloud.google.com/apis/library).

### 2. Configure the OAuth consent screen as Internal

In **APIs & Services → OAuth consent screen**, set the **User type** to
**Internal**. This restricts the app to your Workspace organization, skips
Google's verification process for the sensitive scopes below, and — most
importantly for a long-lived CLI — avoids the 7-day refresh-token expiry
that applies to unverified External/Testing apps.

### 3. Create an OAuth client (Desktop app) and download credentials

In **APIs & Services → Credentials**, create an OAuth 2.0 Client ID of type
**Desktop app**. Download the resulting credentials JSON; it contains
`client_id` and `client_secret`.

### 4. Write the app credentials file

Create `$XDG_CONFIG_HOME/google-chat-cli/app.json` (typically
`~/.config/google-chat-cli/app.json`):

```json
{
  "client_id": "your-client-id",
  "client_secret": "your-client-secret"
}
```

This file is static and hand-written (or written by `google-chat init`) —
the CLI never modifies it. It's kept separate from `credentials.json` (the
dynamic token store, see below) so automatic token writes never overwrite
your app identity.

### 5. (Optional, for agent-driven usage) Set up the service account

`google-chat auth login` (no flags) is the non-interactive, agent-friendly
mode: it impersonates a dedicated Workspace "service user" account via
domain-wide delegation, with no browser involved. This requires a one-time
setup by a Workspace **super-admin**:

1. In Google Cloud Console, create a service account (IAM & Admin → Service
   Accounts) and download its JSON key.
2. On that service account, check **Enable Google Workspace Domain-wide
   Delegation** and note its numeric OAuth Client ID.
3. In the Google Admin Console (admin.google.com → Security → Access and
   data control → API controls → Domain-wide delegation), add that Client
   ID authorized for exactly these scopes (comma-separated):
   `https://www.googleapis.com/auth/chat.spaces.readonly,https://www.googleapis.com/auth/chat.messages.readonly,https://www.googleapis.com/auth/chat.messages.create,https://www.googleapis.com/auth/chat.memberships.readonly,https://www.googleapis.com/auth/pubsub`
4. Add a `service_account` block to `app.json`, using `client_email` and
   `private_key` from the downloaded key, and `impersonate_user` set to the
   service user's email:
   ```json
   {
     "client_id": "your-client-id",
     "client_secret": "your-client-secret",
     "service_account": {
       "client_email": "bot@your-project.iam.gserviceaccount.com",
       "private_key": "-----BEGIN PRIVATE KEY-----\n...\n-----END PRIVATE KEY-----\n",
       "impersonate_user": "service-user@your-workspace.example.com"
     }
   }
   ```

If you skip this, use `auth login --user` instead (step 6) every time — no
admin setup needed, but a human must complete the browser consent flow.

### 6. Log in

```sh
cargo run -p google-chat -- init
```

(or `cargo run -p google-chat -- auth login` / `auth login --user` if
`app.json` is already set up)

`auth login` (no flags) uses the service account from step 5, silently. `auth
login --user` opens Google's consent screen in your browser, listing the
requested scopes (`chat.spaces.readonly`, `chat.messages.readonly`,
`chat.messages.create`, `chat.memberships.readonly`, `pubsub`).
`google-chat init` does step 4 plus the `--user`
login together: it prints setup instructions, prompts for Client ID and
Client Secret, writes `app.json`, runs the interactive OAuth flow, and
finally runs `google-chat doctor` as a confirmation.

## How the OAuth flow works

The CLI supports two OAuth 2.0 grant types, both ultimately authorizing the
same Chat API scopes.

### Service account login (default): domain-wide delegation

`google-chat auth login` (no flags) impersonates the configured Workspace
service user, non-interactively:

1. **JWT assertion** — the CLI builds a JWT (RFC 7523) with `iss` set to the
   service account's `client_email`, `sub` set to `impersonate_user`,
   `scope` set to the Chat API scopes, and `aud` set to the token endpoint;
   it signs this with the service account's RS256 private key.
2. **Token exchange** — the CLI POSTs
   `grant_type=urn:ietf:params:oauth:grant-type:jwt-bearer&assertion=<jwt>`
   (form-urlencoded) to `https://oauth2.googleapis.com/token`, receiving an
   `access_token` and expiry. No `refresh_token` is issued.
3. **Persisting credentials** — `access_token` and `expires_at` are written
   to `credentials.json`; `refresh_token` is omitted/`null`.

This is the expected mode for agent-driven usage: fast, no browser, no human
interaction. Requires the one-time domain-wide-delegation setup (step 5
above) to have been completed by a Workspace super-admin.

### Human login: OAuth 2.0 Authorization Code + PKCE — `auth login --user`

The standard flow for installed apps that can't keep a client secret fully
safe.

1. **Authorization request** — the CLI generates a PKCE `code_verifier`
   (random string) and its `code_challenge` (SHA-256 + base64url), plus a
   random `state` value (CSRF protection). It builds the authorization URL
   with these, the requested scopes, and
   `redirect_uri=http://localhost:8080/callback`, then opens it in the
   browser.
2. **Local callback** — the CLI binds a TCP listener on `127.0.0.1:8080` and
   waits for exactly one request. After you approve access in the browser,
   Google redirects to `http://localhost:8080/callback?code=...&state=...`.
   The CLI parses this, checks `state` matches (aborting on mismatch), and
   replies with a small HTML confirmation page.
3. **Token exchange** — the CLI POSTs (form-urlencoded) the authorization
   `code`, the PKCE `code_verifier`, and the app's
   `client_id`/`client_secret` to `https://oauth2.googleapis.com/token`,
   receiving an `access_token`, `refresh_token`, and expiry.
4. **Persisting credentials** — `access_token`, `refresh_token`, and
   `expires_at` (unix timestamp) are written to `credentials.json`.

### Automatic renewal

Before each API call, the CLI checks whether the access token is expired (or
about to expire within 60s). How it renews depends on whether the stored
credentials have a `refresh_token`:

- **3LO credentials** (`refresh_token` present) — exchanges it for a new
  access token via the `refresh_token` grant and overwrites
  `credentials.json` with the new value. For an Internal-consent-screen app,
  Google does not rotate or expire the refresh token itself on a fixed
  schedule.
- **Service-account credentials** (`refresh_token` absent) — re-signs a
  fresh JWT assertion and re-runs the domain-wide-delegation exchange to get
  a new access token.

## Usage

### `google-chat init`

Interactive onboarding. Prints setup instructions, prompts for Client ID and
Client Secret (or accepts `--client-id`/`--client-secret` flags for
non-interactive use), writes `app.json`, runs the OAuth login flow, and
prints a `google-chat doctor` JSON report as final confirmation.

```sh
cargo run -p google-chat -- init
cargo run -p google-chat -- init --client-id <ID> --client-secret <SECRET>
```

### `google-chat doctor`

Runs three checks and prints a structured JSON report: `app_config` (app.json
exists and is valid), `credentials` (tokens exist and are not expired,
renewing transparently if they are), `api` (live call to `spaces.list` with
`pageSize=1`). Exits non-zero if any check fails. Unlike jira, there is no
separate OAuth-scopes/permissions layer — Google Chat authorizes purely by
scope, with no per-site permission system to probe independently.

```sh
cargo run -p google-chat -- doctor
cargo run -p google-chat -- doctor --select app_config.status,credentials.status,api.status
```

### `google-chat auth login`

Stores credentials locally. By default runs the non-interactive
domain-wide-delegation flow (service account impersonating the configured
Workspace user) — no browser, no human interaction. Pass `--user` for the
interactive OAuth 2.0 Authorization Code + PKCE flow for a human Google
account.

```sh
cargo run -p google-chat -- auth login              # service account (domain-wide delegation)
cargo run -p google-chat -- auth login --user       # human account (OAuth 2.0 + PKCE)
```

Run this once per machine, or again if `credentials.json` is lost or
revoked. The default flow requires `app.json`'s `service_account` block to
be set up (see Setup step 5); without it, use `--user`.

### `google-chat spaces list`

Lists spaces (group chats, DMs, named spaces) the authenticated identity
belongs to. Returns `{"spaces": [...], "nextPageToken": "..."}`. Requires the
`chat.spaces.readonly` scope (already requested by `auth login`).

```sh
cargo run -p google-chat -- spaces list
cargo run -p google-chat -- spaces list --page-size 20
cargo run -p google-chat -- spaces list --page-token <TOKEN>
cargo run -p google-chat -- spaces list --select spaces.name,spaces.displayName,spaces.spaceType
```

**Flags:**
- `--page-size <N>` — maximum number of spaces to return (default 100; the server may return fewer)
- `--page-token <TOKEN>` — cursor for the next page, taken from `nextPageToken` in a previous response

Each space has a `spaceType` of `SPACE` (named space), `GROUP_CHAT`, or
`DIRECT_MESSAGE`. Direct messages and most group chats have no
`displayName`.

### `google-chat messages list --space <id>`

Lists messages in a space. Returns `{"messages": [...], "nextPageToken": "..."}`.
Requires the `chat.messages.readonly` scope (already requested by `auth login`).

Defaults to chronological order (`createTime ASC`, the Chat API's own
default) — this is what makes it useful as a context-recovery tool: page
forward through `--page-token` to walk a space's full history after a gap or
aggressive conversation summarization, rather than only seeing a fixed-size
tail.

```sh
cargo run -p google-chat -- messages list --space spaces/AAQA-_d58OQ
cargo run -p google-chat -- messages list --space AAQA-_d58OQ --page-size 20
cargo run -p google-chat -- messages list --space AAQA-_d58OQ --order-by "createTime DESC"
cargo run -p google-chat -- messages list --space AAQA-_d58OQ --select messages.text,messages.createTime
```

**Flags:**
- `--space <ID>` (required) — bare space id or full `spaces/{id}` resource name, as printed in `spaces list`'s `name` field
- `--page-size <N>` — maximum number of messages to return (default 100; the server may return fewer)
- `--page-token <TOKEN>` — cursor for the next page, taken from `nextPageToken` in a previous response
- `--order-by <ORDER>` — `"createTime ASC"` (default) or `"createTime DESC"` to get the most recent messages first

### `google-chat messages send --space <id> --text <text>`

Sends a plain-text message to a space and prints the created Message
resource as JSON, including its `name` field (needed to identify the message
in future calls). Requires the `chat.messages.create` scope (already
requested by `auth login`).

**This creates real, visible state** — the message appears immediately to
everyone in the target space. Not gated by `--confirm`: unlike deleting data,
sending a message isn't irreversible destruction, just ordinary chat
activity.

```sh
cargo run -p google-chat -- messages send --space spaces/AAQA-_d58OQ --text "Status update: deploy complete"
cargo run -p google-chat -- messages send --space AAQA-_d58OQ --text "Same thing, bare space id"
```

**Flags:**
- `--space <ID>` (required) — bare space id or full `spaces/{id}` resource name
- `--text <TEXT>` (required) — plain-text message body

### `google-chat subscription create`

Registers a [Workspace Events API](https://developers.google.com/workspace/events)
subscription that pushes Chat events for a space to a Pub/Sub topic, so
`google-chat listen` (below) can stream them. Requires the
`chat.spaces.readonly`, `chat.memberships.readonly`, and `pubsub` scopes —
re-run `auth login --user` if you logged in before these were added.

It also ensures a pull subscription exists on the given Pub/Sub topic,
creating one if missing (a no-op if it already exists) — you don't need to
check beforehand whether one was set up in a previous step.

```sh
# Scoped to one space's messages via a Pub/Sub attribute filter:
cargo run -p google-chat -- subscription create --space spaces/AAQA-_d58OQ --topic projects/my-project/topics/my-topic --pubsub-subscription projects/my-project/subscriptions/my-sub --message-filter 'hasPrefix(attributes.ce-subject, "//chat.googleapis.com/spaces/AAQA-_d58OQ")'

# Scoped to two spaces sharing one subscription (OR filter):
cargo run -p google-chat -- subscription create --space spaces/AAQA-_d58OQ --topic projects/my-project/topics/my-topic --pubsub-subscription projects/my-project/subscriptions/my-sub --message-filter 'hasPrefix(attributes.ce-subject, "//chat.googleapis.com/spaces/AAQA-_d58OQ") OR hasPrefix(attributes.ce-subject, "//chat.googleapis.com/spaces/OTHER_SPACE_ID")'

# Explicit opt-out, no filtering:
cargo run -p google-chat -- subscription create --space spaces/AAQA-_d58OQ --topic projects/my-project/topics/my-topic --pubsub-subscription projects/my-project/subscriptions/my-sub --allow-unfiltered
```

**Flags:**
- `--space <ID>` (required) — bare space id or full `spaces/{id}` resource name
- `--topic <TOPIC>` (required) — Pub/Sub topic that will receive events: `projects/{project}/topics/{topic}`
- `--pubsub-subscription <SUBSCRIPTION>` (required) — pull subscription on that topic, created if missing: `projects/{project}/subscriptions/{subscription}`
- `--event-type <TYPE>` (repeatable) — Chat event type to subscribe to; default `google.workspace.chat.message.v1.created`. Other valid values: `.updated`, `.deleted`
- `--message-filter <FILTER>` — Pub/Sub filter expression applied to the pull subscription, so only matching messages are delivered; see [Pub/Sub subscription filters](https://cloud.google.com/pubsub/docs/subscription-message-filter) for syntax. **One of `--message-filter` or `--allow-unfiltered` is required** (same "required unless explicitly confirmed" pattern as `--select`/`--select-all` — an unfiltered subscription silently delivers events for every space ever attached to it, which can flood an agent's `listen` stream with messages from conversations it isn't part of).
- `--allow-unfiltered` — explicitly confirm an unfiltered subscription instead of passing `--message-filter`.

**Scoping to a single space:** `hasPrefix(attributes.ce-subject, "//chat.googleapis.com/spaces/SPACE_ID")` — the space id lives in the `ce-subject` CloudEvents attribute (**not** `ce-source`, which instead holds the Workspace Events subscription's own resource name). Attribute access must use dot notation (`attributes.ce-subject`); Pub/Sub's filter grammar rejects bracket indexing (`attributes["ce-subject"]`) with a parse error, even though the attribute key itself contains a hyphen.

**Scoping to multiple spaces:** since `--message-filter` is passed straight through as Pub/Sub's `filter` field with no interpretation by this CLI, any valid Pub/Sub filter expression works — combine several `hasPrefix(...)` clauses with `OR` to have one subscription (and one `listen` process) deliver events for several spaces at once.

The Pub/Sub topic itself, and the IAM grant of `roles/pubsub.publisher` on it
to `chat-api-push@system.gserviceaccount.com` (required by the Workspace
Events API to publish to it), are **not** created by this command — set
those up once via the Cloud Console or `gcloud` before running it.

**`--topic` and `--message-filter` are immutable once the pull subscription
is created.** If `--pubsub-subscription` already exists with a different
`--topic` or `--message-filter` than requested, the command fails instead of
silently keeping the original configuration — delete the subscription or use
a different `--pubsub-subscription` name to apply the new configuration.

This creates a real tradeoff between two ways to handle multiple concurrent
conversations:
- **One dedicated `--pubsub-subscription` per space**, each with its own
  `listen` process. Starting a new conversation only creates a new
  subscription/process — already-active ones are untouched. Costs one
  `listen` process per active conversation.
- **One shared `--pubsub-subscription`** with an OR filter covering every
  active space, and a single `listen` process. Starting or ending a
  conversation means deleting and recreating that one subscription with an
  updated filter — which briefly interrupts delivery for every
  already-active conversation sharing it, not just the one changing.

Neither is enforced by the CLI; pick based on how disruptive a brief
`listen` restart is for your use case.

**The created subscription expires after ~4 hours** (the Workspace Events
API's own default TTL) — confirmed live, not configurable by this command
yet. Pass its `name` field (printed in the output) to `google-chat listen
--workspace-events-subscription` below, which renews it automatically so
you don't have to re-run `subscription create` (see `BACKLOG.md` GCHAT-4).

### `google-chat subscription delete --name <name>`

Deletes a Workspace Events subscription so it stops delivering events
immediately, rather than waiting for it to expire on its own (~4h, or never,
if a `listen` process is still renewing it). Use this when an agent is done
with a conversation, to tighten access back down to exactly the
conversations actually in progress — important if subscriptions are created
per-space (rather than the broader `--space spaces/-` wildcard, not
recommended: it grants visibility into every space the identity belongs to,
not just the ones the agent is actively engaged in).

```sh
cargo run -p google-chat -- subscription delete --name subscriptions/chat-spaces-abc123
```

**Flags:**
- `--name <NAME>` (required) — the `name` field from `subscription create`'s output: `subscriptions/{id}`

Deleting an already-deleted (or nonexistent) subscription returns a `403
PERMISSION_DENIED` with `reason: SUBSCRIPTION_ACCESS_DENIED` — Workspace
Events conflates "doesn't exist" with "no permission" in this error, so it's
not distinguishable from this CLI's output alone.

### `google-chat listen --pubsub-subscription <name> --workspace-events-subscription <name>`

Opens a streaming pull on a Pub/Sub subscription (created via `subscription
create` above) and prints each received message as one JSON line (NDJSON) to
stdout, then acknowledges it. Pair the two commands to watch a space in real
time instead of polling `messages list`.

Runs until interrupted — Ctrl+C (SIGINT) in a foreground terminal, or
`kill <pid>`/`pkill` (SIGTERM) for a background process, which is the
expected way for an agent or script controlling the process to stop
listening. The PID is printed to stderr at startup for this purpose.
Refreshes its own access token in the background as needed (so it can run
past the ~1h token lifetime), and renews the Workspace Events subscription's
TTL every 30 minutes (so it can run past the ~4h subscription lifetime) —
both without being restarted.

```sh
cargo run -p google-chat -- listen --pubsub-subscription projects/my-project/subscriptions/my-sub --workspace-events-subscription subscriptions/chat-spaces-abc123
```

**Flags:**
- `--pubsub-subscription <SUBSCRIPTION>` (required) — full resource name: `projects/{project}/subscriptions/{subscription}`
- `--workspace-events-subscription <NAME>` (required) — the `name` field from `subscription create`'s output (`subscriptions/{id}`), kept renewed automatically
- `--max-messages <N>` — exit automatically after receiving N messages, instead of running until interrupted (useful for smoke tests)

This is the one async corner of the crate — `google-cloud-pubsub` is
tokio-async only, so `listen` builds and runs its own tokio runtime
internally. Every other command stays on plain blocking `reqwest`.

### `--select <PATHS>` (global flag)

All commands that return JSON support a `--select` flag for client-side
field projection. Pass a comma-separated list of dot-notation paths; only
those paths are included in the output. If omitted, the full response from
the Chat API is printed.

## Testing

### Unit tests

No external dependencies. Run with:

```sh
cargo test -p google-chat
```

### End-to-end tests

E2e tests call the real Google Chat API. They are all marked `#[ignore]` and
never run as part of the normal test suite. Unlike jira, coverage is
deliberately **read-only**: `spaces list` and `messages list` only.
`messages send` creates real, visible messages in spaces shared with real
people, so it's covered only by manual `cargo run` testing during
development, not by an automated test (see `BACKLOG.md` GCHAT-2).
`subscription create` (creates real GCP subscriptions) and `listen` (a
long-running process) are likewise only covered by manual testing, not
automated e2e tests (see `BACKLOG.md` GCHAT-3).

**Prerequisites:** `google-chat auth login --user` (or `init`) must have been
completed on this machine.

**Running:**

```sh
cargo test -p google-chat -- --ignored
```

No isolation/cleanup step is needed — these tests create nothing.

## Error design

All errors are plain text, no colors or symbols — designed to be read by an
LLM. Each message is self-contained: it states what went wrong and what to
do next. Errors are typed with `thiserror` (`CliError` in `error.rs`).
Internal module errors are mapped to `CliError` at the top-level `run()`
function and never surface directly to the user.
