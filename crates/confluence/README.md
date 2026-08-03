# confluence

CLI for Confluence Cloud, designed to be driven by an LLM agent (output is JSON, errors are actionable). This README documents it for humans setting it up and maintaining it; new commands get documented here as they're added.

## Table of contents

- [Setup](#setup)
- [How the OAuth flow works](#how-the-oauth-flow-works)
- [Usage](#usage)
  - [`confluence init`](#confluence-init)
  - [`confluence doctor`](#confluence-doctor)
  - [`confluence auth login`](#confluence-auth-login)
  - [`confluence auth whoami`](#confluence-auth-whoami)
  - [`confluence page get <ID>`](#confluence-page-get-id)
  - [`confluence page create`](#confluence-page-create)
  - [`confluence page update <ID>`](#confluence-page-update-id)
  - [`confluence page search --cql <QUERY>`](#confluence-page-search---cql-query)
  - [`confluence page delete <ID>`](#confluence-page-delete-id)
  - [`confluence space list`](#confluence-space-list)
  - [`confluence template create`](#confluence-template-create)
  - [`confluence template list`](#confluence-template-list)
  - [`confluence template update <ID>`](#confluence-template-update-id)
  - [`confluence template delete <ID>`](#confluence-template-delete-id)
  - [`--select <PATHS>` (global flag)](#--select-paths-global-flag)
- [Testing](#testing)
- [Error design](#error-design)

## Setup

This crate authenticates against the exact same Atlassian OAuth platform as `jira` (see root `CLAUDE.md`'s "Shared library: crates/atlassian-auth") — if you've already set up `jira`, the process here is identical, just with a separate `app.json`/`credentials.json` under `confluence-cli/` instead of `jira-cli/`, and Confluence-specific scopes.

Whatever the source, the CLI ends up needing `client_id`/`client_secret` written to `app.json`, used by `confluence auth login` to get tokens (`credentials.json`). There are two ways to obtain that pair — see `jira`'s README "Setup" section for the full walkthrough of both (identical steps, just substitute "Confluence" for "Jira" in scope names and `confluence-cli` for `jira-cli` in paths):

- **Option A — Service Account (recommended for agent-driven usage)**: generated in Atlassian's admin console (admin.atlassian.com → Directory → Service accounts → Create credentials → OAuth 2.0), with site access assigned by an org admin at generation time. No human consent step ever needed. Select scopes matching `confluence auth login`'s `SCOPES` constant (`crates/confluence/src/auth.rs`) — see this crate's `CLAUDE.md` "OAuth / auth design" for the exact list and why it mixes classic and granular scopes. **Do not run `confluence init`** for this option — see the warning below.
- **Option B — 3LO app (human login)**: register an OAuth 2.0 app at developer.atlassian.com/console/myapps, **Resource-level** access, callback URL `http://localhost:8080/callback`, same scopes as above. Requires one human browser consent before first use — this is what `confluence init` (or `confluence auth login --user`) is for.

> **If you already have a Service Account (Option A), skip `confluence init` entirely.** `init` always ends by launching the interactive 3LO browser-consent flow, which a Service Account doesn't have and doesn't need — running it will just sit there waiting for a browser step that isn't part of this flow. Write `app.json` by hand instead (below) and go straight to `confluence auth login`.

Write `$XDG_CONFIG_HOME/confluence-cli/app.json` (typically `~/.config/confluence-cli/app.json`) yourself — this file holds a secret, so the CLI (and any agent driving it) should never be asked to read or type it in for you:

```json
{
  "client_id": "your-client-id",
  "client_secret": "your-client-secret"
}
```

Then, day-to-day (either option, once `app.json` is in place):

```sh
cargo run -p confluence -- auth login
cargo run -p confluence -- doctor
```

## How the OAuth flow works

Identical mechanics to `jira` — same `auth.atlassian.com`/`api.atlassian.com` endpoints, same `client_credentials` (default, agent-driven) and 3LO+PKCE (`--user`, human) grants, same `cloud_id` resolution via the accessible-resources endpoint, same automatic token renewal before every API call. See `jira`'s README "How the OAuth flow works" section for the full step-by-step — this crate's `auth.rs` is a thin wrapper over the same `atlassian_auth` crate `jira` uses (see this crate's `CLAUDE.md`).

The one difference worth calling out: this crate's OAuth scopes are **not yet live-verified** against a real Confluence site (unlike `jira`'s, which were confirmed end-to-end). See this crate's `CLAUDE.md` "OAuth / auth design" for the scope table and what to check if a command 403s despite `doctor` reporting `oauth_scopes: ok`.

## Usage

### `confluence init`

Interactive onboarding — **only for Option B (3LO app) from Setup.** It always ends by running the interactive browser consent flow, because that flow is what "installs" a 3LO app's access to a Confluence site — there is no way to skip it. **If you're setting up a Service Account (Option A), don't run this command**: write `app.json` by hand (Setup, above) and run `confluence auth login` directly instead.

Prints setup instructions, prompts for Client ID/Secret (or accepts `--client-id`/`--client-secret` flags), writes `app.json`, runs the OAuth login flow, and prints a `confluence doctor` JSON report as confirmation.

```sh
cargo run -p confluence -- init
cargo run -p confluence -- init --client-id <ID> --client-secret <SECRET>
```

### `confluence doctor`

Runs four checks and prints a structured JSON report: `app_config`, `credentials`, `api` (live call to `/wiki/rest/api/user/current`), `oauth_scopes` (granted OAuth scopes via the accessible-resources endpoint). Exits non-zero if any check fails.

```sh
cargo run -p confluence -- doctor
cargo run -p confluence -- doctor --select app_config.status,credentials.status,api.status
```

Unlike `jira doctor`, there is no per-space permission-scheme check yet — see this crate's `CLAUDE.md` "Known gaps".

### `confluence auth login`

Stores credentials locally. By default runs the non-interactive `client_credentials` flow (service account). Pass `--user` for the interactive OAuth 2.0 (3LO) + PKCE flow.

```sh
cargo run -p confluence -- auth login              # service account (client_credentials)
cargo run -p confluence -- auth login --user       # human account (OAuth 2.0 3LO + PKCE)
```

### `confluence auth whoami`

Prints the currently authenticated user as JSON (`GET /wiki/rest/api/user/current`).

```sh
cargo run -p confluence -- auth whoami
```

### `confluence page get <ID>`

Fetches a single page by its numeric ID, including its body in storage format, and prints the full API response as JSON.

```sh
cargo run -p confluence -- page get 123456 --select title,body.storage.value,version.number
```

The response includes a read-only `position` field (a page's order among its siblings). Confluence Cloud has **no public API to change it** — only the UI's drag-and-drop can reorder pages (this is a longstanding, still-open Atlassian feature request: [CONFCLOUD-40101](https://jira.atlassian.com/browse/CONFCLOUD-40101)). This crate has no `page move`/reorder command as a result — there's nothing for it to call.

### `confluence page create`

Creates a page in a space. Requires `--space-id` and `--title`, plus exactly one of `--body`, `--body-file`, or `--template-id` to supply the content — see this crate's `CLAUDE.md` "API design notes" for why `--template-id` works this way (Confluence has no API to create a page "from" a template directly).

```sh
cargo run -p confluence -- page create --space-id 98765 --title "Sprint Notes" --body "<p>Agenda</p>"
cargo run -p confluence -- page create --space-id 98765 --title "Runbook" --body-file ./runbook-content.html
cargo run -p confluence -- page create --space-id 98765 --title "Retro" --template-id 4321 --parent-id 111222
```

`--body`/`--body-file` are raw Confluence **storage format** (XHTML) — the same format `page get`'s `body.storage.value` returns. Plain text with no markup is also valid storage format. `--body-file` is just `--body` read from a local file instead of the command line (handy for longer content) — it has no relation to Confluence's own Template feature, unlike `--template-id`.

### `confluence page update <ID>`

Updates a page's title and/or body. At least one of `--title`/`--body` is required. Fetches the current page first to fill in whichever field you didn't override, and increments the version number automatically (Confluence's v2 API has no partial-patch endpoint).

```sh
cargo run -p confluence -- page update 123456 --title "Sprint Notes (updated)"
cargo run -p confluence -- page update 123456 --body "<p>New agenda</p>"
```

### `confluence page search --cql <QUERY>`

Searches content using CQL (Confluence Query Language). Raw query passed straight through, same approach as `jira issue search --jql`.

```sh
cargo run -p confluence -- page search --cql "type=page AND space=ENG AND title~\"Runbook\""
cargo run -p confluence -- page search --cql "type=page AND space=ENG" --limit 10
```

**Flags:** `--limit <N>` (default 25), `--start <N>` (offset for pagination, default 0).

### `confluence page delete <ID>`

Deletes a page. Requires `--confirm`. By default this **moves the page to the trash** — recoverable, not permanent. Pass `--purge` to permanently remove it instead, but this only works on a page that's already trashed: to fully delete a page, call this command twice — once without `--purge`, then again with it.

```sh
cargo run -p confluence -- page delete 123456 --confirm
cargo run -p confluence -- page delete 123456 --confirm --purge
```

Prints `{"deleted": true, "id": "123456", "purged": false}` on success (synthesized by the CLI — Confluence itself returns 204 No Content).

### `confluence space list`

Lists Confluence spaces, cursor-paginated.

```sh
cargo run -p confluence -- space list
cargo run -p confluence -- space list --limit 10
cargo run -p confluence -- space list --cursor <cursor-from-previous-response>
```

### `confluence template create`

Creates a content template. Requires `--name`, plus exactly one of `--body`/`--body-file` to supply the content (same storage-format XHTML as `page create` — see that command's section above). Omit `--space-key` for a global template (requires Confluence Administrator global permission); pass it for a space template (requires Admin permission on that space).

```sh
cargo run -p confluence -- template create --name "Runbook" --space-key ENG --body "<p>Steps</p>"
cargo run -p confluence -- template create --name "Postmortem" --body-file ./postmortem.html --description "Standard postmortem layout"
```

The created template's `templateId` can be passed to [`page create --template-id`](#confluence-page-create) to build pages from it.

### `confluence template list`

Lists content templates, offset-paginated. Omit `--space-key` to list global templates; pass it to scope to one space.

```sh
cargo run -p confluence -- template list
cargo run -p confluence -- template list --space-key ENG
cargo run -p confluence -- template list --limit 10 --start 10
```

### `confluence template update <ID>`

Updates a template's name, description, and/or body. At least one of `--name`/`--description`/`--body`/`--body-file` is required. Fetches the current template first to fill in whichever fields you didn't override (Confluence's template API has no partial-patch endpoint, same as `page update`).

```sh
cargo run -p confluence -- template update 4321 --name "Runbook (v2)"
cargo run -p confluence -- template update 4321 --body-file ./runbook-v2.html
```

### `confluence template delete <ID>`

Permanently deletes a template. Requires `--confirm` — unlike page delete, this is not a soft delete; there's no trash for templates.

```sh
cargo run -p confluence -- template delete 4321 --confirm
```

Prints `{"deleted": true, "id": "4321"}` on success.

### `--select <PATHS>` (global flag)

Same client-side field-projection flag as every other crate in this workspace — see root `CLAUDE.md`'s "Shared library: crates/cli-fields". Mandatory on every command in this crate except `doctor` and `auth whoami` — omitting both `--select` and `--select-all` fails with the response's byte size and top-level field names instead of printing.

```sh
cargo run -p confluence -- page get 123456 --select title,version.number
cargo run -p confluence -- space list --select results.id,results.key,results.name
```

## Testing

### Unit tests

No external dependencies. Run with:

```sh
cargo test -p confluence
```

### End-to-end tests

None yet — this crate has not been exercised against a real Confluence site. See this crate's `CLAUDE.md` "Known gaps" for what adding them (following `jira`'s `IssueGuard`-style pattern) will look like.

## Error design

All errors are plain text, no colors or symbols — designed to be read by an LLM. Each message is self-contained: it states what went wrong and what to do next. Example:

```
not authenticated. Run: confluence auth login
```

Errors are typed with `thiserror` (`CliError` in `error.rs`). Internal module errors (`LoginError`, `ClientError`) are mapped to `CliError` at the top-level `run()` function and never surface directly to the user.
