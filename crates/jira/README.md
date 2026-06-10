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

### 4. Grant the app access to your site (one-time, human step)

The app you registered in step 1 has no access to any Jira site until a human explicitly grants it, by completing the consent screen once:

```sh
cargo run -p jira -- init
```

(or `cargo run -p jira -- auth login --user` if `app.json` is already set up)

This opens the Atlassian **consent screen** in your browser, listing the site(s) the app is requesting access to (`read:jira-work read:jira-user write:jira-work offline_access`). **Approving this is the actual "install"/authorization step** — it's what makes the site show up in `https://api.atlassian.com/oauth/token/accessible-resources`, which both grant types use to resolve `cloud_id`.

`jira init` does this plus steps 2 and writes `app.json` for you: it prints setup instructions, prompts for Client ID and Client Secret, writes `app.json`, runs this consent flow, and finally runs `jira doctor` as a confirmation.

You must do this **at least once per Atlassian site**, signed in as a user who has access to that site. Until then, every login attempt — including the default `client_credentials` one below — fails (no accessible resources).

### 5. Day-to-day login

Once step 4 has been completed, day-to-day login (e.g. for an agent) is non-interactive:

```sh
cargo run -p jira -- auth login
```

You only need to do this once per machine — after that, the CLI renews tokens automatically (see below).

## How the OAuth flow works

The CLI supports two OAuth 2.0 grant types, both using the same `client_id`/`client_secret` from `app.json`.

### Service account login (default): `client_credentials`

`jira auth login` (no flags) requests an access token directly:

1. **Token request** — the CLI POSTs `grant_type=client_credentials`, `client_id`, `client_secret`, and `audience=api.atlassian.com` to `https://auth.atlassian.com/oauth/token`. No browser, no user interaction. Receives an `access_token` and expiry (no `refresh_token`).
2. **Cloud ID resolution** — same as below: `https://api.atlassian.com/oauth/token/accessible-resources` with the new access token.
3. **Persisting credentials** — `access_token`, `expires_at`, and `cloud_id` are written to `credentials.json` (`refresh_token` is omitted/`null`).

This is the expected mode for agent-driven usage: fast, no human interaction, and the resulting account has `accountType: "app"` (visible via `jira auth whoami`).

This requires the OAuth app to already have been granted access to a Jira site — i.e. step 4 above (`jira init` / `jira auth login --user`) must have been completed at least once.

### Human login: OAuth 2.0 (3LO) + PKCE — `jira auth login --user` or `jira init`

The standard flow for apps that can't keep a secret fully safe (a CLI binary on a user's machine), combined with a confidential client (since Atlassian 3LO apps do issue a client secret).

1. **Authorization request** — the CLI generates a PKCE `code_verifier` (random string) and its `code_challenge` (SHA-256 + base64url), plus a random `state` value (CSRF protection). It builds the authorization URL with these, the requested scopes (`read:jira-work read:jira-user write:jira-work offline_access`), and `redirect_uri=http://localhost:8080/callback`, then opens it in the browser.
2. **Local callback** — the CLI binds a TCP listener on `127.0.0.1:8080` and waits for exactly one request. After you approve access in the browser, Atlassian redirects to `http://localhost:8080/callback?code=...&state=...`. The CLI parses this, checks `state` matches (aborting on mismatch — a sign of a hijacked flow), and replies with a small HTML confirmation page.
3. **Token exchange** — the CLI POSTs the authorization `code`, the PKCE `code_verifier`, and the app's `client_id`/`client_secret` to `https://auth.atlassian.com/oauth/token`, receiving an `access_token`, `refresh_token`, and expiry.
4. **Cloud ID resolution** — Jira's OAuth API is accessed through `https://api.atlassian.com/ex/jira/<cloud_id>/...`, not the site's own URL. The CLI calls `https://api.atlassian.com/oauth/token/accessible-resources` with the new access token to discover the `cloud_id` of the authorized site.
5. **Persisting credentials** — `access_token`, `refresh_token`, `expires_at` (unix timestamp), and `cloud_id` are written to `credentials.json`.

### Automatic renewal

Before each API call, the CLI checks whether the access token is expired (or about to expire within 60s). How it renews depends on whether the stored credentials have a `refresh_token`:

- **3LO credentials** (`refresh_token` present) — exchanges it for a new token pair via the `refresh_token` grant and **overwrites** `credentials.json` with the new values. **Atlassian refresh tokens rotate on every use**: each refresh invalidates the previous refresh token and issues a new one. The CLI always persists the freshest pair — if you copy `credentials.json` to another machine and both machines try to refresh independently, one will end up with a stale, invalidated token.
- **Service account credentials** (`refresh_token` absent) — re-runs the `client_credentials` token request to get a fresh access token.

## Usage

### `jira init`

Interactive onboarding. Prints setup instructions, prompts for Client ID and Client Secret (or accepts `--client-id`/`--client-secret` flags for non-interactive use), writes `app.json`, runs the OAuth login flow, and prints a `jira doctor` JSON report as final confirmation.

```sh
cargo run -p jira -- init
cargo run -p jira -- init --client-id <ID> --client-secret <SECRET>
```

### `jira doctor`

Runs four checks and prints a structured JSON report: `app_config` (app.json exists and is valid), `credentials` (tokens exist and are not expired), `api` (live call to Jira succeeds), `permissions` (actual Jira permissions granted to the account, via `/rest/api/3/mypermissions`). Exits non-zero if any check fails.

```sh
cargo run -p jira -- doctor
cargo run -p jira -- doctor --select app_config.status,credentials.status,api.status,permissions
```

The `permissions` check reports `BROWSE_PROJECTS`, `CREATE_ISSUES`, `EDIT_ISSUES`, `DELETE_ISSUES`, `ADD_COMMENTS`, and `TRANSITION_ISSUES`. `status` is `"ok"` only if `BROWSE_PROJECTS` is granted (without it no `issue` command works); the others are reported informationally. Note: these are **global** permission checks (no project context), so a permission can show `false` here while still being usable on specific projects you have access to — if an `issue` command unexpectedly fails with a permission error, check this project's permissions directly in Jira.

### `jira auth login`

Stores credentials locally. By default runs the non-interactive `client_credentials` flow (service account) — no browser, no human interaction. Pass `--user` for the interactive OAuth 2.0 (3LO) + PKCE flow for a human Atlassian account.

```sh
cargo run -p jira -- auth login              # service account (client_credentials)
cargo run -p jira -- auth login --user       # human account (OAuth 2.0 3LO + PKCE)
```

Run this once per machine, or again if `credentials.json` is lost or revoked. The `--user` flow must have been completed at least once (e.g. via `jira init`) before the default flow can succeed.

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

### `jira issue create`

Creates a new issue. Required: `--project`, `--type`, `--summary`. Optional: `--description`, `--assignee`, `--priority`.

```sh
cargo run -p jira -- issue create --project KAN --type Task --summary "Fix login bug"
cargo run -p jira -- issue create --project KAN --type Bug --summary "Crash on startup" \
  --description "Reproducible on macOS 14" --priority High
```

Prints the Jira response (`id`, `key`, `self`) on success.

### `jira issue delete <KEY>`

Permanently deletes an issue. Requires `--confirm` as an explicit acknowledgement — omitting it prints an error with the exact command to run. If the issue has subtasks, also pass `--delete-subtasks` (Jira returns 400 otherwise).

```sh
cargo run -p jira -- issue delete KAN-5 --confirm
cargo run -p jira -- issue delete KAN-5 --confirm --delete-subtasks
```

Prints `{"deleted": true, "key": "KAN-5"}` on success.

### `jira issue transitions <KEY>`

Lists the workflow transitions available for an issue in its current state, as raw JSON.

```sh
cargo run -p jira -- issue transitions KAN-4
```

Useful before `issue transition` to discover valid target states. Use `--select transitions.id,transitions.name` to get a compact list.

### `jira issue transition <KEY> --to <STATUS>`

Moves an issue to a different workflow state. The `--to` value is matched case-insensitively against the available transition names. If the name doesn't match, the error lists the valid options.

```sh
cargo run -p jira -- issue transition KAN-4 --to "In Progress"
cargo run -p jira -- issue transition KAN-4 --to done
```

Prints `{"transitioned": true, "key": "KAN-4", "to": "In Progress"}` on success.

### `jira issue comment add <KEY> --body <TEXT>`

Adds a plain-text comment to an issue (converted to Jira's document format internally). Prints the created comment as JSON.

```sh
cargo run -p jira -- issue comment add KAN-4 --body "Blocked by network issue, retrying tomorrow"
```

### `jira issue comment remove <KEY> <COMMENT_ID>`

Deletes a comment by ID (the `id` field in the comment JSON from `comment add` or `issue get`). Prints `{"deleted": true, "id": "..."}` on success.

```sh
cargo run -p jira -- issue comment remove KAN-4 10033
```

### `jira issue search --jql <QUERY>`

Searches issues using JQL (Jira Query Language). Returns the raw response including `issues`, `isLast`, and `nextPageToken` (when more pages exist).

```sh
cargo run -p jira -- issue search --jql "project=KAN AND status=\"In Progress\""
cargo run -p jira -- issue search --jql "project=KAN" --fields summary,status,priority --max-results 10
```

**Flags:**
- `--max-results <N>` — how many issues to return (default 50, max 100)
- `--fields <NAMES>` — comma-separated Jira field names to include per issue (server-side, reduces payload). Use `*all` for every field, `*navigable` for defaults. Example: `summary,status,assignee,priority`
- `--page-token <TOKEN>` — cursor for the next page, taken from `nextPageToken` in a previous response

Combine `--fields` (server-side) with `--select` (client-side) for maximum control:

```sh
cargo run -p jira -- issue search --jql "project=KAN" \
  --fields summary,status \
  --select issues.key,issues.fields.summary,issues.fields.status.name,isLast
```

### `--select <PATHS>` (global flag)

All commands that return JSON support a `--select` flag for client-side field projection. Pass a comma-separated list of dot-notation paths; only those paths are included in the output. If omitted, the full response from Jira is printed.

```sh
# compact transitions list
cargo run -p jira -- issue transitions KAN-4 --select transitions.id,transitions.name

# just the key fields of an issue
cargo run -p jira -- issue get KAN-4 --select summary,status.name,assignee.displayName

# only your account details
cargo run -p jira -- auth whoami --select accountId,displayName,emailAddress
```

The flag can appear before or after the subcommand. Arrays (like `transitions`) are projected element-wise automatically — no special syntax needed.

## Testing

### Unit tests

No external dependencies. Run with:

```sh
cargo test -p jira
```

### End-to-end tests

E2e tests call the real Jira API. They are all marked `#[ignore]` and never run as part of the normal test suite.

**Prerequisites:**

1. `jira auth login` must have been completed on this machine.
2. A writable Jira project must exist. Set its key via the `JIRA_E2E_PROJECT` environment variable (e.g. `KAN`). The project must allow creating and deleting Task issues.

**Running:**

```sh
# Run all e2e tests (sequentially — see note below)
JIRA_E2E_PROJECT=KAN cargo test -p jira -- --ignored --test-threads=1

# Run a single test
JIRA_E2E_PROJECT=KAN cargo test -p jira e2e_smoke_doctor -- --ignored
```

> **Note:** use `--test-threads=1`. The search tests run JQL queries scoped to the whole project (e.g. for pagination); when other tests create/delete issues concurrently, those queries can return inconsistent results.

**Isolation:** every issue created by the tests has the `[jira-cli-e2e]` prefix in its summary. An `IssueGuard` (RAII) deletes each issue on drop, so cleanup happens even when a test panics. If a test is interrupted before the guard is set up, run the recovery command:

```sh
JIRA_E2E_PROJECT=KAN cargo test -p jira e2e_cleanup -- --ignored
```

This searches for all `[jira-cli-e2e]` issues in the project and deletes them.

## Error design

All errors are plain text, no colors or symbols — designed to be read by an LLM. Each message is self-contained: it states what went wrong and what to do next. Example:

```
not authenticated. Run: jira auth login
```

Errors are typed with `thiserror` (`CliError` in `error.rs`). Internal module errors (`LoginError`, `ClientError`) are mapped to `CliError` at the top-level `run()` function and never surface directly to the user.