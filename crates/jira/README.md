# jira

CLI for Jira Cloud, designed to be driven by an LLM agent (output is JSON, errors are actionable). This README documents it for humans setting it up and maintaining it; new commands get documented here as they're added.

## Setup

### 1. Create an Atlassian OAuth 2.0 (3LO) app

Go to [developer.atlassian.com/console/myapps](https://developer.atlassian.com/console/myapps/) and create a new **OAuth 2.0 integration**:

- **Callback/redirect URI**: `http://localhost:8080/callback`
- **Permissions**: enable Jira API access with scopes `read:jira-work` and `read:jira-user` (more will be added as commands grow)

From the app's **Settings** page, note down the **Client ID** and **Client Secret**.

> The third scope the CLI requests, `offline_access`, doesn't need to be enabled in the console — it's requested at runtime via the authorization URL and is what makes the refresh token possible.

### 2. Write the app credentials file

Create `$XDG_CONFIG_HOME/jira-cli/app.json` (typically `~/.config/jira-cli/app.json`):

```json
{
  "client_id": "your-client-id",
  "client_secret": "your-client-secret"
}
```

This file is static and hand-written — the CLI never modifies it. It's kept separate from `credentials.json` (the dynamic token store, see below) precisely so that automatic token writes never overwrite your app identity.

### 3. Make sure the Atlassian account has a Jira site

The OAuth account you log in with must have access to at least one Jira Cloud site (e.g. `your-name.atlassian.net`). If it doesn't, authorization fails with "Access denied — this app requires access to a Jira site...". Create a free site at [atlassian.com/software/jira/free](https://www.atlassian.com/software/jira/free) if needed.

### 4. Log in

```sh
cargo run -p jira -- auth login
```

This prints an authorization URL and opens it in your browser. Approve access on the Atlassian consent screen; the CLI runs a one-shot local server on `localhost:8080` to receive the callback, exchanges the authorization code for tokens, and writes `$XDG_CONFIG_HOME/jira-cli/credentials.json`.

You only need to do this once per machine — after that, the CLI refreshes tokens automatically (see below).

## How the OAuth flow works

The CLI uses **OAuth 2.0 (3LO) with PKCE** — the standard flow for apps that can't keep a secret fully safe (a CLI binary on a user's machine), combined with a confidential client (since Atlassian 3LO apps do issue a client secret).

1. **Authorization request** — the CLI generates a PKCE `code_verifier` (random string) and its `code_challenge` (SHA-256 + base64url), plus a random `state` value (CSRF protection). It builds the authorization URL with these, the requested scopes (`read:jira-work read:jira-user offline_access`), and `redirect_uri=http://localhost:8080/callback`, then opens it in the browser.
2. **Local callback** — the CLI binds a TCP listener on `127.0.0.1:8080` and waits for exactly one request. After you approve access in the browser, Atlassian redirects to `http://localhost:8080/callback?code=...&state=...`. The CLI parses this, checks `state` matches (aborting on mismatch — a sign of a hijacked flow), and replies with a small HTML confirmation page.
3. **Token exchange** — the CLI POSTs the authorization `code`, the PKCE `code_verifier`, and the app's `client_id`/`client_secret` to `https://auth.atlassian.com/oauth/token`, receiving an `access_token`, `refresh_token`, and expiry.
4. **Cloud ID resolution** — Jira's OAuth API is accessed through `https://api.atlassian.com/ex/jira/<cloud_id>/...`, not the site's own URL. The CLI calls `https://api.atlassian.com/oauth/token/accessible-resources` with the new access token to discover the `cloud_id` of the authorized site.
5. **Persisting credentials** — `access_token`, `refresh_token`, `expires_at` (unix timestamp), and `cloud_id` are written to `credentials.json`.

### Automatic refresh

Before each API call, the CLI checks whether the access token is expired (or about to expire within 60s). If so, it exchanges the `refresh_token` for a new token pair via the `refresh_token` grant and **overwrites** `credentials.json` with the new values.

This matters because **Atlassian refresh tokens rotate on every use**: each refresh invalidates the previous refresh token and issues a new one. The CLI always persists the freshest pair — if you copy `credentials.json` to another machine and both machines try to refresh independently, one will end up with a stale, invalidated token.

## Usage

### `jira auth login`

Runs the interactive OAuth login described above and stores credentials locally. Run this once per machine, or again if `credentials.json` is lost or revoked.

### `jira auth whoami`

Prints the currently authenticated user as JSON. Useful to verify that authentication is working and to discover the `accountId` of the authenticated user (needed to filter issues by assignee).

```sh
cargo run -p jira -- auth whoami
```

### `jira issue get <KEY>`

Fetches a single issue by its key (e.g. `KAN-4`) and prints the full Jira API response as pretty-printed JSON to stdout.

```sh
cargo run -p jira -- issue get KAN-4
```

On error (issue not found, not authenticated, etc.), prints a message to stderr and exits non-zero. If not authenticated, the hint points you to `jira auth login`.

## Error design

All errors are plain text, no colors or symbols — designed to be read by an LLM. Each message is self-contained: it states what went wrong and what to do next. Example:

```
not authenticated. Run: jira auth login
```

Errors are typed with `thiserror` (`CliError` in `error.rs`). Internal module errors (`LoginError`, `ClientError`) are mapped to `CliError` at the top-level `run()` function and never surface directly to the user.
