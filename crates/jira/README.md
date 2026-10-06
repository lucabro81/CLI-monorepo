# jira

CLI for Jira Cloud, designed to be driven by an LLM agent (output is JSON, errors are actionable). This README documents it for humans setting it up and maintaining it; new commands get documented here as they're added.

## Table of contents

- [Setup](#setup)
- [How the OAuth flow works](#how-the-oauth-flow-works)
- [Usage](#usage)
  - [`jira init`](#jira-init)
  - [`jira doctor`](#jira-doctor)
  - [`jira auth login`](#jira-auth-login)
  - [`jira auth whoami`](#jira-auth-whoami)
  - [`jira issue get <KEY>`](#jira-issue-get-key)
  - [`jira issue create`](#jira-issue-create)
  - [`jira issue delete <KEY>`](#jira-issue-delete-key)
  - [`jira issue transitions <KEY>`](#jira-issue-transitions-key)
  - [`jira issue transition <KEY> --to <STATUS>`](#jira-issue-transition-key---to-status)
  - [`jira issue assign <KEY>`](#jira-issue-assign-key)
  - [`jira issue comment add <KEY> --body <TEXT>`](#jira-issue-comment-add-key---body-text)
  - [`jira issue comment remove <KEY> <COMMENT_ID>`](#jira-issue-comment-remove-key-comment_id)
  - [`jira issue search --jql <QUERY>`](#jira-issue-search---jql-query)
  - [`jira user search --query <TEXT>`](#jira-user-search---query-text)
  - [`jira project search --query <TEXT>`](#jira-project-search---query-text)
  - [`--select <PATHS>` (global flag)](#--select-paths-global-flag)
- [Testing](#testing)
- [Error design](#error-design)

## Setup

The CLI holds two identities side by side, and every command picks one per call:

- **Service Account (the default identity)** — what every command acts as without `--user`. Generated in Atlassian's admin console, with site access assigned by an org admin at generation time; no human ever needs to authorize anything — verified end-to-end against a real org: `jira auth login` and `jira doctor` (all six checks) succeed immediately with no browser step.
- **Human (`--user`)** — what a command acts as when given `--user`. Needs a 3LO app registered in the developer console, plus one human completing a browser consent. Only needed if you want an interactive human identity in addition to the agent identity.

Both live in one config folder, `$XDG_CONFIG_HOME/jira-cli/` (typically `~/.config/jira-cli/`):

| File | Written by | Holds |
|---|---|---|
| `app.json` | `jira init` / `jira init --user` | `"service"` section (Service Account Client ID/Secret) and `"user"` section (3LO app Client ID/Secret) — either may be missing |
| `credentials-service.json` | `jira auth login` | the Service Account's token |
| `credentials-user.json` | `jira auth login --user` | the human's token and refresh token |

```json
{
  "service": { "client_id": "service-account-client-id", "client_secret": "service-account-client-secret" },
  "user":    { "client_id": "3lo-app-client-id",        "client_secret": "3lo-app-client-secret" }
}
```

Each `init`/`auth login` touches only its own section and file, so setting up or logging in as one identity never logs the other out. An `app.json` in the old flat format (`client_id` at top level, before issue #164) is rejected with the commands to recreate it; a leftover `credentials.json` is ignored and reported by `jira doctor` so it can be deleted.

### Service Account (the default identity, no human login ever)

Requires an Atlassian **organization** (admin.atlassian.com) — a different, org-wide console from developer.atlassian.com's per-developer app console used for the human identity.

1. Go to [admin.atlassian.com](https://admin.atlassian.com) → **Directory** → **Service accounts**. (Free tier includes a small number of free service accounts; Atlassian Guard Standard/Enterprise unlocks more. Check your org's current limit in the console if you're unsure.)
2. Create (or select an existing) service account.
3. On the service account, click **Create credentials** → **OAuth 2.0**.
4. Select scopes: the Jira product scopes matching this CLI's needs — `read:jira-work`, `read:jira-user`, `write:jira-work`. (`offline_access` doesn't apply here: `client_credentials` never returns a refresh token, by design — see "Automatic renewal" below.)
5. Copy the generated **Client ID** and **Client Secret** immediately — they are shown once.
6. Save them, log in and verify — no browser involved:

   ```sh
   cargo run -p jira -- init --client-id <ID> --client-secret <SECRET>
   ```

   `init` writes the `"service"` section of `app.json`, runs `jira auth login` and prints a `jira doctor` report.

Site access for this credential was already assigned by whoever set up the service account in steps 1–3, in the admin console itself — there is no separate "grant access" step.

### Human identity: 3LO app (`--user`)

Only needed for commands run with `--user`.

Go to [developer.atlassian.com/console/myapps](https://developer.atlassian.com/console/myapps/) and create a new **OAuth 2.0 integration**:

- **Access type**: select **Resource-level**, not Account-level. This CLI only supports a single Jira site: `fetch_primary_resource` (`atlassian-auth` crate) takes the first entry returned by the accessible-resources endpoint and assumes it's the only one. Resource-level matches this — the consent screen limits the grant (and what `accessible-resources` returns) to the one site the user selects. Account-level would let one consent cover every site in the user's Atlassian account, which this codebase doesn't handle: multiple accessible sites would make `fetch_primary_resource` silently pick an arbitrary one. Supporting Account-level (letting the user or config pick which site to target) is a separate, deliberate change — not a setup detail to work around here.
- **Callback/redirect URI**: `http://localhost:8080/callback` (add more lines for [remote logins](#remote-login-in-two-steps--jira-auth-login---user---remote))
- **Permissions**: enable Jira API access with scopes `read:jira-work`, `read:jira-user` and `write:jira-work`

From the app's **Settings** page, note down the **Client ID** and **Client Secret**.

> The fourth scope the CLI requests, `offline_access`, doesn't need to be enabled in the console — it's requested at runtime via the authorization URL and is what makes the refresh token possible.

Make sure the Atlassian account you'll log in with has access to at least one Jira Cloud site (e.g. `your-name.atlassian.net`). If it doesn't, authorization fails with "Access denied — this app requires access to a Jira site...". Create a free site at [atlassian.com/software/jira/free](https://www.atlassian.com/software/jira/free) if needed.

This app has **no** access to any Jira site until a human grants it, by completing the consent screen once:

```sh
cargo run -p jira -- init --user --client-id <ID> --client-secret <SECRET>
```

(or `cargo run -p jira -- auth login --user` if the `"user"` section is already set up)

This writes the `"user"` section of `app.json`, then opens the Atlassian **consent screen** in your browser, listing the site(s) the app is requesting access to (`read:jira-work read:jira-user write:jira-work offline_access`). **Approving this is the actual "install"/authorization step** — it's what makes the site show up in `https://api.atlassian.com/oauth/token/accessible-resources`. Finally it prints a `jira doctor --user` report.

### Day-to-day use

Once logged in, neither identity needs a new login: tokens are renewed automatically (see below). An agent picks the identity on every call:

```sh
cargo run -p jira -- issue get PROJ-1 --select key          # as the Service Account
cargo run -p jira -- issue get PROJ-1 --select key --user   # as the human
```

Only when the human's refresh token expires or is revoked does a person need to run `jira auth login --user` again; every error says which login to run.

## How the OAuth flow works

The CLI supports two OAuth 2.0 grant types, one per identity: `client_credentials` with `app.json`'s `"service"` section, 3LO + PKCE with its `"user"` section.

### Service account login (default): `client_credentials`

`jira auth login` (no flags) requests an access token directly:

1. **Token request** — the CLI POSTs `grant_type=client_credentials`, `client_id`, `client_secret`, and `audience=api.atlassian.com` to `https://auth.atlassian.com/oauth/token`. No browser, no user interaction. Receives an `access_token` and expiry (no `refresh_token`).
2. **Cloud ID resolution** — same as below: `https://api.atlassian.com/oauth/token/accessible-resources` with the new access token.
3. **Persisting credentials** — `access_token`, `expires_at`, and `cloud_id` are written to `credentials-service.json` (`refresh_token` is omitted/`null`).

This is the expected mode for agent-driven usage: fast, no human interaction, and the resulting account has `accountType: "app"` (visible via `jira auth whoami`).

With a Service Account credential in the `"service"` section this works immediately: site access was already assigned by an org admin when the credential was created. (A 3LO app's credentials there would also work with `client_credentials`, but only after a one-time human consent for that app — Atlassian ties a 3LO app's site access to that authorization; without it the call fails with "no accessible resources".)

### Human login: OAuth 2.0 (3LO) + PKCE — `jira auth login --user` or `jira init --user`

The standard flow for apps that can't keep a secret fully safe (a CLI binary on a user's machine), combined with a confidential client (since Atlassian 3LO apps do issue a client secret).

1. **Authorization request** — the CLI generates a PKCE `code_verifier` (random string) and its `code_challenge` (SHA-256 + base64url), plus a random `state` value (CSRF protection). It builds the authorization URL with these, the requested scopes (`read:jira-work read:jira-user write:jira-work offline_access`), and `redirect_uri=http://localhost:8080/callback`, then opens it in the browser.
2. **Local callback** — the CLI binds a TCP listener on `127.0.0.1:8080` (before opening the browser, so a busy port fails right away) and waits for the callback. After you approve access in the browser, Atlassian redirects to `http://localhost:8080/callback?code=...&state=...`. Stray requests such as `/favicon.ico` get a 404 and the CLI keeps waiting. The CLI checks `state` matches (aborting on mismatch — a sign of a hijacked flow), reports a denied consent as such, and replies with a short plain-text page.
3. **Token exchange** — the CLI POSTs the authorization `code`, the PKCE `code_verifier`, and the app's `client_id`/`client_secret` to `https://auth.atlassian.com/oauth/token`, receiving an `access_token`, `refresh_token`, and expiry.
4. **Cloud ID resolution** — Jira's OAuth API is accessed through `https://api.atlassian.com/ex/jira/<cloud_id>/...`, not the site's own URL. The CLI calls `https://api.atlassian.com/oauth/token/accessible-resources` with the new access token to discover the `cloud_id` of the authorized site.
5. **Persisting credentials** — `access_token`, `refresh_token`, `expires_at` (unix timestamp), and `cloud_id` are written to `credentials-user.json`.

### Remote login, in two steps — `jira auth login --user --remote`

The same 3LO grant for a person who is not at the CLI's machine (the CLI runs on a server, the person is in a chat or a web page). Nothing opens a browser or listens on a port; whoever runs the CLI carries the link to the person and the code back. Uses the `"user"` section (a 3LO app): a Service Account credential can't do a user login at all.

1. `jira auth login --user --remote --redirect-uri <url>` stores a pending login (`state`, PKCE verifier, redirect URI; `pending-login.json`, mode `0600`) and prints `{"authorize_url", "state", "expires_at"}`. `<url>` must be one of the 3LO app's callback URLs (the console's Callback URL field takes one per line).
2. The person opens `authorize_url`, picks the site and accepts; Atlassian redirects them to `<url>?code=...&state=...`.
3. `jira auth login --user --code <code> --state <state>` checks the state and the expiry, exchanges the code with the stored verifier and redirect URI, resolves the `cloud_id`, saves `credentials-user.json`, and prints what `auth whoami --user` prints.

`--remote`, `--code` and `--state` always need `--user` (before or after `auth login`); without it the CLI answers with the corrected commands. A pending login is valid for 10 minutes and its state is single-use (consumed even if Atlassian then refuses the code). A new step 1 replaces the previous pending login. Everything lives in the config folder the CLI resolves, so pointing `XDG_CONFIG_HOME` at one folder per person keeps people's logins apart. `jira doctor` shows a pending login under `pending_login`. For other people to log in, the 3LO app must have sharing enabled (developer console → Distribution).

### Automatic renewal

Before each API call, the CLI checks whether the selected identity's access token is expired (or about to expire within 60s). How it renews depends on whether those credentials have a `refresh_token`:

- **Human credentials** (`credentials-user.json`, `refresh_token` present) — exchanges it for a new token pair via the `refresh_token` grant and **overwrites** `credentials-user.json` with the new values. **Atlassian refresh tokens rotate on every use**: each refresh invalidates the previous refresh token and issues a new one. The CLI always persists the freshest pair — if you copy the file to another machine and both machines try to refresh independently, one will end up with a stale, invalidated token.
- **Service account credentials** (`credentials-service.json`, `refresh_token` absent) — re-runs the `client_credentials` token request to get a fresh access token.

## Usage

Every command accepts the global `--user` flag: without it the command acts as the Service Account, with it as the human (see [Setup](#setup)). Examples below omit it unless it changes what the command does.

### `jira init`

Onboarding for one identity. Prints setup instructions, prompts for Client ID and Client Secret (or accepts `--client-id`/`--client-secret` flags for non-interactive use), writes that identity's section of `app.json` (leaving the other section alone), logs in, and prints a `jira doctor` JSON report for that identity as final confirmation.

- `jira init` — the Service Account: writes `"service"`, runs the non-interactive `client_credentials` login.
- `jira init --user` — the human: writes `"user"`, runs the browser consent flow.

```sh
cargo run -p jira -- init --client-id <ID> --client-secret <SECRET>          # Service Account
cargo run -p jira -- init --user --client-id <ID> --client-secret <SECRET>   # 3LO app, browser login
```

### `jira doctor`

Runs its checks for the selected identity (the Service Account, or the human with `--user`) and prints a structured JSON report: `app_config` (app.json exists and has the identity's section), `credentials` (that identity's tokens exist and are not expired), `api` (live call to Jira succeeds), `oauth_scopes`, `service_user` (global permissions of the account), `projects` (per-project permissions and roles). Exits non-zero if any check fails. Two informational keys never affect the exit code: `pending_login` (a remote login waiting for its code) and `identities` (which identity was checked, whether each identity has a credentials file, and whether a pre-#164 `credentials.json` is still there).

```sh
cargo run -p jira -- doctor
cargo run -p jira -- doctor --user
cargo run -p jira -- doctor --select app_config.status,credentials.status,api.status,identities
```

The `service_user` check reports which of `BROWSE_PROJECTS`, `CREATE_ISSUES`, `EDIT_ISSUES`, `DELETE_ISSUES`, `ADD_COMMENTS`, `TRANSITION_ISSUES`, `USER_PICKER` and `ASSIGN_ISSUES` the account holds. These are **global** permission checks (no project context), so a permission can show `false` here while still being usable on specific projects — the `projects` check lists them per project.

### `jira auth login`

Logs in one identity and stores its credentials, leaving the other identity's untouched. By default runs the non-interactive `client_credentials` flow for the Service Account (`credentials-service.json`) — no browser, no human interaction. Pass `--user` for the interactive OAuth 2.0 (3LO) + PKCE flow for a human Atlassian account (`credentials-user.json`), or `--user --remote` for the [two-step remote login](#remote-login-in-two-steps--jira-auth-login---user---remote) (step 1 prints `{authorize_url, state, expires_at}`, step 2 prints the `auth whoami --user` output).

```sh
cargo run -p jira -- auth login              # Service Account (client_credentials)
cargo run -p jira -- auth login --user       # human account (OAuth 2.0 3LO + PKCE)
cargo run -p jira -- auth login --user --remote --redirect-uri https://app.example.com/oauth/callback   # step 1
cargo run -p jira -- auth login --user --code <CODE> --state <STATE>                                   # step 2
```

Run each once per machine, or again if that identity's credentials file is lost or revoked.

### `jira auth whoami`

Prints the account the CLI acts as, as JSON: the Service Account, or the human with `--user`. Useful to verify that authentication is working. That account is also what JQL's `currentUser()` resolves to — so to filter issues by a person, look up their `accountId` with [`jira user search`](#jira-user-search---query-text) and use it explicitly (e.g. `--jql "assignee=5b10ac8d82e05b22cc7d4ef5"`).

```sh
cargo run -p jira -- auth whoami
cargo run -p jira -- auth whoami --user
```

### `jira issue get <KEY>`

Fetches a single issue by its key (e.g. `KAN-4`) and prints it as pretty-printed JSON to stdout. `--select` is required: issues carry arbitrary per-project custom fields, so even one issue can be large (see [`--select`](#--select-paths-global-flag)).

```sh
cargo run -p jira -- issue get KAN-4 --select key,fields.summary,fields.status.name
cargo run -p jira -- issue get KAN-4 --select fields.summary,fields.status.name,fields.assignee.displayName,browse_url
```

On error (issue not found, not authenticated, etc.), prints a message to stderr and exits non-zero. If not authenticated, the hint points you to `jira auth login` (or `jira auth login --user` when run with `--user`).

### `jira issue create`

Creates a new issue. Required: `--project`, `--type`, `--summary`. Optional: `--description`, `--assignee`, `--priority`.

`--description` accepts Markdown, converted to Jira's document format: headings (`#`…`######`), bullet/numbered lists, inline `` `code` `` and fenced ` ```code blocks``` `, `**bold**`, `_italic_`/`*italic*`, `[links](url)`, and line breaks (a blank line starts a new paragraph, a single line break becomes a hard break within one).

```sh
cargo run -p jira -- issue create --project KAN --type Task --summary "Fix login bug"
cargo run -p jira -- issue create --project KAN --type Bug --summary "Crash on startup" \
  --description "Reproducible on macOS 14" --priority High
cargo run -p jira -- issue create --project KAN --type Task --summary "Add caching" \
  --description "## Plan

- profile the endpoint
- add a cache layer

See \`get_data()\`."
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

### `jira issue assign <KEY>`

Assigns an issue to a user, or unassigns it. Exactly one of `--assignee` or `--unassign` must be passed — passing both is rejected at parse time, passing neither is rejected at runtime with an error listing both retry commands. Requires the "Assign Issues" project permission (`ASSIGN_ISSUES`, checked by `jira doctor`).

```sh
cargo run -p jira -- issue assign KAN-4 --assignee 5b10ac8d82e05b22cc7d4ef5
cargo run -p jira -- issue assign KAN-4 --unassign
```

Use [`jira user search`](#jira-user-search---query-text) to find an account ID. Prints `{"assigned": true, "key": "KAN-4", "assignee": "5b10ac8d82e05b22cc7d4ef5"}` (or `"assignee": null` after `--unassign`) on success.

### `jira issue comment add <KEY> --body <TEXT>`

Adds a comment to an issue. `--body` accepts Markdown, converted to Jira's document format the same way as `issue create --description` (see above). Prints the created comment as JSON.

```sh
cargo run -p jira -- issue comment add KAN-4 --body "Blocked by network issue, retrying tomorrow"
cargo run -p jira -- issue comment add KAN-4 --body "Root cause:

- stale cache
- missing invalidation on \`update()\`"
```

Two ways to tag/mention a user in the comment, which can be combined:

```sh
# --mention tags a user at the start of the comment
cargo run -p jira -- issue comment add KAN-4 --mention 5b10ac8d82e05b22cc7d4ef5 --body "can you take a look?"

# {{mention:ACCOUNT_ID}} inside --body tags a user at that exact position in the text
cargo run -p jira -- issue comment add KAN-4 --body "Thanks {{mention:5b10ac8d82e05b22cc7d4ef5}} for the fix"
```

Use [`jira user search`](#jira-user-search---query-text) to find the account ID to mention. Both forms resolve the user's current display name via a `GET /rest/api/3/user` lookup before building the comment, so a failure to resolve the account ID (e.g. it doesn't exist) surfaces as the same Jira API error as any other request.

### `jira issue comment remove <KEY> <COMMENT_ID>`

Deletes a comment by ID (the `id` field in the comment JSON from `comment add` or `issue get`). Prints `{"deleted": true, "id": "..."}` on success.

```sh
cargo run -p jira -- issue comment remove KAN-4 10033
```

### `jira issue search --jql <QUERY>`

Searches issues using JQL (Jira Query Language). The response has `issues`, `isLast`, and `nextPageToken` (when more pages exist). `--select` is required (see [`--select`](#--select-paths-global-flag)); `--fields` does not replace it.

```sh
cargo run -p jira -- issue search --jql "project=KAN AND status=\"In Progress\"" \
  --select issues.key,issues.fields.summary
cargo run -p jira -- issue search --jql "project=KAN" --fields summary,status,priority --max-results 10 \
  --select issues.key,issues.fields.summary,issues.fields.status.name,issues.fields.priority.name,nextPageToken
cargo run -p jira -- issue search --jql "project=KAN AND status!=Done" --stale-days 14 \
  --select issues.key,issues.fields.summary,issues.fields.updated
```

**Flags:**
- `--max-results <N>` — how many issues to return (default 50, max 100)
- `--fields <NAMES>` — comma-separated Jira field names to include per issue (server-side, reduces payload). Use `*all` for every field, `*navigable` for defaults. Example: `summary,status,assignee,priority`
- `--page-token <TOKEN>` — cursor for the next page, taken from `nextPageToken` in a previous response
- `--stale-days <N>` — only include issues not updated in at least N days. Appends `AND updated <= -Nd` to `--jql` server-side (JQL's own relative-date syntax, evaluated by Jira — inserted before `ORDER BY` if your `--jql` already sorts results)

Combine `--fields` (server-side) with `--select` (client-side) for maximum control:

```sh
cargo run -p jira -- issue search --jql "project=KAN" \
  --fields summary,status \
  --select issues.key,issues.fields.summary,issues.fields.status.name,isLast
```

### `jira user search --query <TEXT>`

Searches for Jira users by name or email fragment. Returns a JSON array of matches (up to Jira's own limit of the first 1000 users). `--select` is required.

```sh
cargo run -p jira -- user search --query "Jane Doe" --select accountId,displayName
cargo run -p jira -- user search --query jane.doe@example.com --select accountId,displayName,emailAddress
```

Requires the "Browse users and groups" global permission. Without it, Jira does not return an error — it silently returns an empty match list. Check `jira doctor`'s permissions report (the `USER_PICKER` key) if searches unexpectedly return nothing.

### `jira project search --query <TEXT>`

Searches for Jira projects by name or key fragment — use this to find a project's key when you only know (part of) its name. `--query` is a literal substring/prefix filter (case-insensitive) against both key and name, not a query language; JQL's `project = <value>` clause can also resolve a project's *exact, full* name to its key, but does not do fragment/substring matching (verified live: `project = Mercury` resolves, `project = mercur` returns zero results).

`--select` is required.

```sh
cargo run -p jira -- project search --query Mercury --select values.key,values.name
cargo run -p jira -- project search --query mercur --select values.key,values.name
```

Use the `key` from the result as the `--project` value for `issue create` or in JQL.

### `--select <PATHS>` (global flag)

Client-side field projection: pass a comma-separated list of dot-notation paths and only those paths are printed. The nested shape is kept, and arrays are projected element-wise.

**`--select` is mandatory** on commands whose output can be large: `issue get`, `issue search`, `user search`, `project search`. Without it they print nothing and exit non-zero, reporting the response's byte size and top-level field names so you can retry with an informed `--select`. To print the whole response anyway, pass `--select-all`. It is still refused above 30000 bytes, and the error reports the actual size and top-level fields.

All other commands are **exempt** and always print their full (small, fixed-shape) result: `doctor`, `auth whoami`, `issue create`, `issue delete`, `issue transitions`, `issue transition`, `issue assign`, `issue comment add`, `issue comment remove`. `--select` still narrows their output if passed.

Paths are relative to the top level of the response and must match its exact structure: a path that doesn't exist is silently dropped, not an error. For example, `issue get KAN-4 --select summary` prints `{}` with exit 0, because the summary lives under `fields.summary`. Use the top-level field names from the refusal message to build the right path.

```sh
# compact transitions list
cargo run -p jira -- issue transitions KAN-4 --select transitions.id,transitions.name

# just the key fields of an issue
cargo run -p jira -- issue get KAN-4 --select fields.summary,fields.status.name,fields.assignee.displayName

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
2. A writable Jira project must exist. Set its key via the `JIRA_E2E_PROJECT` environment variable (e.g. `MER`). The project must allow creating and deleting Task issues.

`JIRA_E2E_PROJECT` can be exported inline per run (as below), or set once in a
workspace-root `.env` file (copy `.env.example`, gitignored) — it's loaded
automatically before every e2e test runs. An inline/exported value always
takes precedence over `.env`.

**Running:**

```sh
# Run all e2e tests (sequentially — see note below)
JIRA_E2E_PROJECT=MER cargo test -p jira -- --ignored --test-threads=1

# Run a single test
JIRA_E2E_PROJECT=MER cargo test -p jira e2e_smoke_doctor -- --ignored

# Same, relying on JIRA_E2E_PROJECT from a workspace-root .env instead:
cargo test -p jira e2e_smoke_doctor -- --ignored
```

> **Note:** use `--test-threads=1`. The search tests run JQL queries scoped to the whole project (e.g. for pagination); when other tests create/delete issues concurrently, those queries can return inconsistent results.

**Isolation:** every issue created by the tests has the `[jira-cli-e2e]` prefix in its summary. An `IssueGuard` (RAII) deletes each issue on drop, so cleanup happens even when a test panics. If a test is interrupted before the guard is set up, run the recovery command:

```sh
JIRA_E2E_PROJECT=MER cargo test -p jira e2e_cleanup -- --ignored
```

This searches for all `[jira-cli-e2e]` issues in the project and deletes them.

> **Note:** both `IssueGuard` and `e2e_cleanup` require the authenticated account to have `DELETE_ISSUES` permission on `JIRA_E2E_PROJECT` (check via `jira doctor`). If that permission is missing, deletes fail with a 403 that is silently swallowed by `IssueGuard::drop` and reported (but not retried) by `e2e_cleanup`. In that case, leftover `[jira-cli-e2e]` issues must be deleted manually from the Jira UI.

## Error design

All errors are plain text, no colors or symbols — designed to be read by an LLM. Each message is self-contained: it states what went wrong and what to do next. Example:

```
not logged in as the service account. Run: jira auth login. To act as the human logged in with jira auth login --user, pass --user instead
```

Errors are typed with `thiserror` (`CliError` in `error.rs`). Internal module errors (`LoginError`, `ClientError`) are mapped to `CliError` at the top-level `run()` function and never surface directly to the user.