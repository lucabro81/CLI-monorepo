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
   `https://www.googleapis.com/auth/chat.spaces.readonly,https://www.googleapis.com/auth/chat.messages.readonly,https://www.googleapis.com/auth/chat.messages.create`
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
`chat.messages.create`). `google-chat init` does step 4 plus the `--user`
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

TODO — decide e2e approach once core commands land (likely manual testing
against the user's real Workspace, given Google Chat has no equivalent of a
disposable "test project" the way Jira does).

## Error design

All errors are plain text, no colors or symbols — designed to be read by an
LLM. Each message is self-contained: it states what went wrong and what to
do next. Errors are typed with `thiserror` (`CliError` in `error.rs`).
Internal module errors are mapped to `CliError` at the top-level `run()`
function and never surface directly to the user.
