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

### 5. Log in

```sh
cargo run -p google-chat -- init
```

(or `cargo run -p google-chat -- auth login` if `app.json` is already set up)

This opens Google's consent screen in your browser, listing the requested
scopes (`chat.spaces.readonly`, `chat.messages.readonly`,
`chat.messages.create`). `google-chat init` does steps 4 and 5 together: it
prints setup instructions, prompts for Client ID and Client Secret, writes
`app.json`, runs the OAuth flow, and finally runs `google-chat doctor` as a
confirmation.

## How the OAuth flow works

The CLI supports a single OAuth 2.0 grant type — Authorization Code + PKCE,
the standard flow for installed apps that can't keep a client secret fully
safe. There is no `client_credentials`/service-account grant: Google Chat's
non-interactive bot/app identity grant is for chat apps, not for reading or
sending messages as a human user, which is this CLI's purpose.

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
3. **Token exchange** — the CLI POSTs the authorization `code`, the PKCE
   `code_verifier`, and the app's `client_id`/`client_secret` to
   `https://oauth2.googleapis.com/token`, receiving an `access_token`,
   `refresh_token`, and expiry.
4. **Persisting credentials** — `access_token`, `refresh_token`, and
   `expires_at` (unix timestamp) are written to `credentials.json`.

### Automatic renewal

Before each API call, the CLI checks whether the access token is expired (or
about to expire within 60s), and if so exchanges the stored `refresh_token`
for a new access token via the same token endpoint, overwriting
`credentials.json` with the new value. For an Internal-consent-screen app,
Google does not rotate or expire the refresh token itself on a fixed
schedule.

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

TODO — filled in as `doctor` is implemented.

### `google-chat auth login`

TODO — filled in as `auth login` is implemented.

### `google-chat auth whoami`

TODO — filled in as `auth whoami` is implemented.

### `google-chat spaces list`

TODO — filled in as `spaces list` is implemented.

### `google-chat messages list --space <id>`

TODO — filled in as `messages list` is implemented.

### `google-chat messages send --space <id> --text <text>`

TODO — filled in as `messages send` is implemented.

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
