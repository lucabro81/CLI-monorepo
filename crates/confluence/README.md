# confluence

CLI for Confluence Cloud, designed to be driven by an LLM agent (output is JSON, errors are actionable). This README documents it for humans setting it up and maintaining it; new commands get documented here as they're added.

## Table of contents

- [Setup](#setup)
- [How the OAuth flow works](#how-the-oauth-flow-works)
- [Usage](#usage)
  - [`confluence init`](#confluence-init)
  - [`confluence doctor`](#confluence-doctor)
  - [`confluence auth login`](#confluence-auth-login)
  - [`confluence auth logout`](#confluence-auth-logout)
  - [`confluence auth whoami`](#confluence-auth-whoami)
  - [`confluence page get <ID>`](#confluence-page-get-id)
  - [`confluence page create`](#confluence-page-create)
  - [`confluence page update <ID>`](#confluence-page-update-id)
  - [`confluence page search --cql <QUERY>`](#confluence-page-search---cql-query)
  - [`confluence page children <ID>`](#confluence-page-children-id)
  - [`confluence page ancestors <ID>`](#confluence-page-ancestors-id)
  - [`confluence page delete <ID>`](#confluence-page-delete-id)
  - [`confluence space get <KEY>`](#confluence-space-get-key)
  - [`confluence space list`](#confluence-space-list)
  - [`confluence template create`](#confluence-template-create)
  - [`confluence template list`](#confluence-template-list)
  - [`confluence template update <ID>`](#confluence-template-update-id)
  - [`confluence template delete <ID>`](#confluence-template-delete-id)
  - [`--select <PATHS>` (global flag)](#--select-paths-global-flag)
- [Testing](#testing)
- [Error design](#error-design)

## Setup

This crate authenticates against the exact same Atlassian OAuth platform as `jira` (see root `CLAUDE.md`'s "Shared library: crates/atlassian-auth") — if you've already set up `jira`, the process here is identical, just under `confluence-cli/` instead of `jira-cli/`, with Confluence-specific scopes. See `jira`'s README "Setup" for the full walkthrough.

The CLI holds the Service Account and any number of people side by side, and every command picks one per call: without `--user` it acts as the **Service Account**, with `--user <USER_ID>` as the **person** who logged in with `confluence auth login --user <USER_ID>`. `<USER_ID>` is your own name for that person, a lowercase slug (`a-z`, `0-9`, `.`, `_`, `-`, `:`, e.g. `jane.doe` or `chat:u123`). All of them live in `$XDG_CONFIG_HOME/confluence-cli/` (typically `~/.config/confluence-cli/`), every file with mode `0600`:

| File | Written by | Holds |
|---|---|---|
| `app.json` | `confluence init` / `confluence init --user <USER_ID>` / `confluence init --user-app` | `"service"` section (Service Account Client ID/Secret) and `"user"` section (3LO app Client ID/Secret, shared by every person) — either may be missing |
| `credentials-service.json` | `confluence auth login` | the Service Account's token |
| `users/<USER_ID>/credentials.json` | `confluence auth login --user <USER_ID>` | that person's token and refresh token |
| `users/<USER_ID>/pending-login.json` | `confluence auth login --user <USER_ID> --remote` | that person's remote login waiting for its code |

An empty `<credentials file>.lock` (mode `0600`) appears next to a credentials file after its first renewal: it keeps parallel commands from renewing the same token twice. Leave it in place.

Each `init`/`auth login` touches only its own section and file; `confluence auth logout [--user <USER_ID>]` removes one identity's login. An `app.json` in the old flat format (`client_id` at top level, before issue #164) is rejected with the commands to recreate it; leftover credentials files of earlier layouts (`credentials.json`, `credentials-user.json`) are ignored and reported by `confluence doctor`.

- **Service Account (the default identity)**: generated in Atlassian's admin console (admin.atlassian.com → Directory → Service accounts → Create credentials → OAuth 2.0), with site access assigned by an org admin at generation time. No human consent step ever needed. Select the scopes `init` prints ("Scopes to add", the `SCOPES` constant in `crates/confluence/src/auth.rs`) — see this crate's `CLAUDE.md` "OAuth / auth design" for the exact list and why it mixes classic and granular scopes. The same Service Account credential can serve `jira` too, scoped to both products.

  ```sh
  cargo run -p confluence -- init --client-id <ID> --client-secret <SECRET>
  ```

- **People (`--user <USER_ID>`)**: register one OAuth 2.0 app for everybody at developer.atlassian.com/console/myapps, **Resource-level** access, callback URL `http://localhost:8080/callback`, Confluence API scopes `read:confluence-user`, `search:confluence`, `read:confluence-content.summary` and `write:confluence-content` under Classic scopes and `read:page:confluence`, `write:page:confluence`, `read:space:confluence`, `read:hierarchical-content:confluence`, `read:content.metadata:confluence` under Granular scopes (`offline_access` is requested by the CLI and needs no setting). Then, with one browser consent:

  ```sh
  cargo run -p confluence -- init --user jane.doe --client-id <ID> --client-secret <SECRET>
  ```

`app.json` holds secrets, so the CLI (and any agent driving it) should never be asked to read it or type the secrets in for you — pass them to `init` yourself.

Further people log in with `confluence auth login --user <USER_ID>`. Day-to-day, no identity needs a new login: tokens are renewed automatically (under a per-file lock, so parallel commands for the same person renew once). An agent picks the identity on every call:

```sh
cargo run -p confluence -- space list --select results.key                   # as the Service Account
cargo run -p confluence -- space list --select results.key --user jane.doe   # as jane.doe
```

**A token revoked before it expires** (e.g. the person ended their session or revoked the app's access; issue #240) only shows up as a `401` from the API. The CLI then renews that identity's token once, under the same lock, and repeats the request (safe: a `401` means the request was not processed). If that renewal is refused, a person's command exits `3` (see [Exit codes](#exit-codes)); a `401` that persists with the fresh token is reported as is.

## How the OAuth flow works

Identical mechanics to `jira` — same `auth.atlassian.com`/`api.atlassian.com` endpoints, same `client_credentials` (default, agent-driven, `"service"` section, `credentials-service.json`) and 3LO+PKCE (`--user <USER_ID>`, a person, `"user"` section, `users/<USER_ID>/credentials.json`) grants, same `cloud_id` resolution via the accessible-resources endpoint, same automatic token renewal before every API call. See `jira`'s README "How the OAuth flow works" section for the full step-by-step — this crate's `auth.rs` is a thin wrapper over the same `atlassian_auth` crate `jira` uses (see this crate's `CLAUDE.md`).

The one difference worth calling out: only five of this crate's OAuth scopes (`read:confluence-user`, `search:confluence`, `read:page:confluence`, `write:page:confluence`, `read:space:confluence`) were checked against a real Service Account credential, and only `auth whoami`/`doctor` were run live; the scopes added in #256 and the page/space/template commands are not live-verified (unlike `jira`'s, confirmed end-to-end). See this crate's `CLAUDE.md` "OAuth / auth design" for the scope table and what to check if a command 403s despite `doctor` reporting `oauth_scopes: ok`.

## Usage

Every command accepts the global `--user <USER_ID>` flag: without it the command acts as the Service Account, with it as that person (see [Setup](#setup)).

### `confluence init`

Onboarding for one identity. Prints setup instructions and the scopes to add, prompts for Client ID/Secret — the secret hidden (no echo) on a terminal, a plain line when stdin is piped; an empty answer writes nothing — (or accepts `--client-id`/`--client-secret` flags), writes that identity's section of `app.json` (leaving the other section alone), logs in, and prints a `confluence doctor` JSON report for that identity.

- `confluence init` — the Service Account: writes `"service"`, runs the non-interactive `client_credentials` login.
- `confluence init --user <USER_ID>` — the 3LO app every person uses: writes `"user"`, runs the browser consent flow for that person.
- `confluence init --user-app` — the same 3LO app, logging nobody in (e.g. in a container nobody sits at): writes `"user"` and prints only `{"app_config": ...}` for that section, after the same setup instructions, then a "Nobody is logged in yet" hint with the commands that log people in (`confluence auth login --user <USER_ID>` in the browser here, or `confluence auth login --user <USER_ID> --remote --redirect-uri <URL>` for a person elsewhere). Takes no `--user`; exits 1 if the section can't be read back.

```sh
cargo run -p confluence -- init --client-id <ID> --client-secret <SECRET>          # Service Account
cargo run -p confluence -- init --user jane.doe --client-id <ID> --client-secret <SECRET>   # 3LO app, jane.doe logs in
cargo run -p confluence -- init --user-app --client-id <ID> --client-secret <SECRET>   # 3LO app only, nobody logged in
```

### `confluence doctor`

Runs four checks for the selected identity (the Service Account, or the person with `--user <USER_ID>`) and prints a structured JSON report: `app_config` (app.json has the identity's section), `credentials` (that identity's tokens), `api` (live call to `/wiki/rest/api/user/current`), `oauth_scopes` (granted OAuth scopes via the accessible-resources endpoint). A token the `api` call gets a `401` for is renewed and the call repeated, as every command does; if that renewal fails, `credentials` reports the error (with the login command) and the later checks are `skipped`. Exits non-zero if any check fails. Two informational keys never affect the exit code: `pending_login` (the selected person's remote login) and `identities` (which identity was checked, whether the Service Account is logged in, the ids of the people logged in under `users`, and credentials files of earlier layouts under `legacy_credentials_files`).

```sh
cargo run -p confluence -- doctor
cargo run -p confluence -- doctor --user jane.doe
cargo run -p confluence -- doctor --select app_config.status,credentials.status,api.status,identities
```

Unlike `jira doctor`, there is no per-space permission-scheme check yet — see this crate's `CLAUDE.md` "Known gaps".

### `confluence auth login`

Logs in one identity and stores its credentials, leaving every other identity's untouched. By default runs the non-interactive `client_credentials` flow for the Service Account (`credentials-service.json`). Pass `--user <USER_ID>` for the interactive OAuth 2.0 (3LO) + PKCE flow for that person (`users/<USER_ID>/credentials.json`).

```sh
cargo run -p confluence -- auth login              # Service Account (client_credentials)
cargo run -p confluence -- auth login --user jane.doe       # a person (OAuth 2.0 3LO + PKCE)
cargo run -p confluence -- auth login --user jane.doe --remote --redirect-uri https://app.example.com/oauth/callback   # step 1
cargo run -p confluence -- auth login --user jane.doe --code <CODE> --state <STATE>                                   # step 2
```

`--user <USER_ID> --remote` is the same 3LO grant in two steps, for a person who is not at the CLI's machine (the CLI on a server, the person in a chat or a web page); it uses the `"user"` section (a 3LO app), not a Service Account credential. `--remote`, `--code` and `--state` always need `--user <USER_ID>` (before or after `auth login`).

1. Step 1 (`--remote --redirect-uri <url>`) opens no browser and listens on no port: it stores a pending login for that person (`state`, PKCE verifier, redirect URI; `users/<USER_ID>/pending-login.json`, mode `0600`) and prints `{"authorize_url", "state", "expires_at"}`. `<url>` must be one of the 3LO app's callback URLs.
2. The person opens `authorize_url`, picks the site and accepts; Atlassian redirects them to `<url>?code=...&state=...`.
3. Step 2 (`--code <code> --state <state>`) checks the state and the expiry, exchanges the code, saves `users/<USER_ID>/credentials.json`, and prints what `auth whoami --user <USER_ID>` prints.

A pending login is valid for 10 minutes and its state is single-use. Each person has their own, so several people can be mid-login at once. `confluence doctor --user <USER_ID>` shows it under `pending_login`.

### `confluence auth logout`

Removes the stored login of the Service Account, or of the person with `--user <USER_ID>` (their whole `users/<USER_ID>/` folder), and prints `{"logged_out": "service"}` or `{"logged_out": "user:<USER_ID>"}`. Other identities and `app.json` are untouched; tokens are not revoked at Atlassian. Fails, naming the login command, if that identity had no stored login.

```sh
cargo run -p confluence -- auth logout --user jane.doe
```

### `confluence auth whoami`

Prints the account the CLI acts as, as JSON (`GET /wiki/rest/api/user/current`): the Service Account, or the person with `--user <USER_ID>`.

```sh
cargo run -p confluence -- auth whoami
cargo run -p confluence -- auth whoami --user jane.doe
```

### `confluence page get <ID>`

Fetches a single page by its numeric ID, including its body, and prints the API response as JSON.

```sh
cargo run -p confluence -- page get 123456 --select title,body.markdown.value
cargo run -p confluence -- page get 123456 --body-format storage --select title,body.storage.value,version.number
```

**`--body-format <markdown|storage>`** (default `markdown`):

- `markdown` — the page as rendered (macros expanded, Confluence's `export_view`) converted to Markdown, in `body.markdown = {"representation": "markdown", "value": "..."}`. For reading: storage XHTML is full of `ac:`/`ri:` tags that cost context and carry little. Tables become GFM tables, the code and noformat macros fenced code blocks; panels and status lozenges keep only their text.
- `storage` — Confluence's storage XHTML, in `body.storage`, as returned by the API. The format `page create`/`page update` `--body` take, so use it to edit a page.

The response includes a read-only `position` field (a page's order among its siblings). Confluence Cloud has **no public API to change it** — only the UI's drag-and-drop can reorder pages (this is a longstanding, still-open Atlassian feature request: [CONFCLOUD-40101](https://jira.atlassian.com/browse/CONFCLOUD-40101)). This crate has no `page move`/reorder command as a result — there's nothing for it to call.

### `confluence page create`

Creates a page in a space. Requires `--space-id` and `--title`, plus exactly one of `--body`, `--body-file`, or `--template-id` to supply the content — see this crate's `CLAUDE.md` "API design notes" for why `--template-id` works this way (Confluence has no API to create a page "from" a template directly).

```sh
cargo run -p confluence -- page create --space-id 98765 --title "Sprint Notes" --body "<p>Agenda</p>" --select id,title,version.number
cargo run -p confluence -- page create --space-id 98765 --title "Runbook" --body-file ./runbook-content.html --select id,title,version.number
cargo run -p confluence -- page create --space-id 98765 --title "Retro" --template-id 4321 --parent-id 111222 --select id,title,version.number
```

`--body`/`--body-file` are raw Confluence **storage format** (XHTML) — the same format `page get --body-format storage` returns in `body.storage.value`. Plain text with no markup is also valid storage format. `--body-file` is just `--body` read from a local file instead of the command line (handy for longer content) — it has no relation to Confluence's own Template feature, unlike `--template-id`.

### `confluence page update <ID>`

Updates a page's title and/or body. At least one of `--title`/`--body` is required. Fetches the current page first to fill in whichever field you didn't override, and increments the version number automatically (Confluence's v2 API has no partial-patch endpoint).

```sh
cargo run -p confluence -- page update 123456 --title "Sprint Notes (updated)" --select id,title,version.number
cargo run -p confluence -- page update 123456 --body "<p>New agenda</p>" --select id,title,version.number
```

### `confluence page search --cql <QUERY>`

Searches content using CQL (Confluence Query Language). Raw query passed straight through, same approach as `jira issue search --jql`. A page result carries `content` (`id`, `type`, `title`), `excerpt` (plain text around the match, highlight markers removed), `url` (relative to the response's `_links.base`), `resultGlobalContainer` (the space) and `lastModified`; a CQL not limited to `type=page` can also return spaces, users or attachments, which have no `content`. An invalid CQL query (Confluence answers 400) fails with Confluence's own message. `--select` is required.

```sh
cargo run -p confluence -- page search --cql "type=page AND space=ENG AND text~\"deploy\"" --select results.content.id,results.content.title,results.excerpt,results.url
cargo run -p confluence -- page search --cql "type=page AND space=ENG" --limit 10 --select results.content.id,results.content.title,_links.base
```

**Flags:** `--limit <N>` (default 25), `--start <N>` (offset for pagination, default 0).

### `confluence page children <ID>`

Lists a page's direct children: pages, folders, whiteboards, databases and embeds. Each child has `id`, `title`, `type`, `status`, `spaceId` and `childPosition` (its order under the parent). Cursor-paginated like `space list`; a space's root page is `space get`'s `homepageId`. `--select` is required.

```sh
cargo run -p confluence -- page children 123456 --select results.id,results.title,results.type
cargo run -p confluence -- page children 123456 --limit 50 --cursor <cursor-from-previous-response> --select results.id,results.title,_links.next
```

**Flags:** `--limit <N>` (default 25), `--cursor <CURSOR>` (from the previous response's `_links.next`).

### `confluence page ancestors <ID>`

Lists up to 250 of a page's ancestors (v2's maximum, one request; deeper paths are cut) in the order Confluence returns them, documented as from the top of the space down to its parent (an empty list for a top-level page). Confluence returns only `id` and `type` per ancestor; the CLI adds the `title` of each page ancestor (one request each), while a folder, or a page ancestor whose lookup answers 404 (restricted or deleted), keeps `id` and `type` only; any other failed lookup (for example a 403) fails the command. `--select` is required.

```sh
cargo run -p confluence -- page ancestors 123456 --select results.id,results.title,results.type
```

Both commands report an unknown page (or one this identity can't see) with an error pointing at `page search`.

**New scopes (#256).** `page children` needs `read:hierarchical-content:confluence`, `page ancestors` needs `read:content.metadata:confluence`, plus `read:page:confluence` (already in `SCOPES`) for its ancestors' titles (all granular); the `template` commands' classic scopes `read:confluence-content.summary` and `write:confluence-content` were added to `SCOPES` at the same time. A login made before them gets a 403 on those commands: for the Service Account, add the scopes to its credential in admin.atlassian.com, then run `confluence auth login`; for people, add them to the 3LO app in the developer console, then each person runs `confluence auth login --user <USER_ID>` again to consent.

### `confluence page delete <ID>`

Deletes a page. Requires `--confirm`. By default this **moves the page to the trash** — recoverable, not permanent. Pass `--purge` to permanently remove it instead, but this only works on a page that's already trashed: to fully delete a page, call this command twice — once without `--purge`, then again with it.

```sh
cargo run -p confluence -- page delete 123456 --confirm
cargo run -p confluence -- page delete 123456 --confirm --purge
```

Prints `{"deleted": true, "id": "123456", "purged": false}` on success (synthesized by the CLI — Confluence itself returns 204 No Content).

### `confluence space get <KEY>`

Gets one space by its key (`ENG`, the form CQL and people use) and prints the space object: `id` (the numeric id `page create --space-id` takes), `key`, `name`, `type`, `status` and `homepageId` (the space's root page, a starting point for `page get`). An unknown key fails with an error pointing at `space list`. `--select` is optional: without it the whole object is printed, with it only the listed fields.

```sh
cargo run -p confluence -- space get ENG
cargo run -p confluence -- space get ENG --select id,homepageId
```

### `confluence space list`

Lists Confluence spaces, cursor-paginated.

```sh
cargo run -p confluence -- space list --select results.id,results.key,results.name,_links.next
cargo run -p confluence -- space list --limit 10 --select results.id,results.key,results.name,_links.next
cargo run -p confluence -- space list --cursor <cursor-from-previous-response> --select results.id,results.key,results.name,_links.next
```

### `confluence template create`

Creates a content template. Requires `--name`, plus exactly one of `--body`/`--body-file` to supply the content (same storage-format XHTML as `page create` — see that command's section above). Omit `--space-key` for a global template (requires Confluence Administrator global permission); pass it for a space template (requires Admin permission on that space).

```sh
cargo run -p confluence -- template create --name "Runbook" --space-key ENG --body "<p>Steps</p>" --select templateId,name
cargo run -p confluence -- template create --name "Postmortem" --body-file ./postmortem.html --description "Standard postmortem layout" --select templateId,name
```

The created template's `templateId` can be passed to [`page create --template-id`](#confluence-page-create) to build pages from it.

### `confluence template list`

Lists content templates, offset-paginated. Omit `--space-key` to list global templates; pass it to scope to one space.

```sh
cargo run -p confluence -- template list --select results.templateId,results.name
cargo run -p confluence -- template list --space-key ENG --select results.templateId,results.name
cargo run -p confluence -- template list --limit 10 --start 10 --select results.templateId,results.name
```

### `confluence template update <ID>`

Updates a template's name, description, and/or body. At least one of `--name`/`--description`/`--body`/`--body-file` is required. Fetches the current template first to fill in whichever fields you didn't override (Confluence's template API has no partial-patch endpoint, same as `page update`).

```sh
cargo run -p confluence -- template update 4321 --name "Runbook (v2)" --select templateId,name
cargo run -p confluence -- template update 4321 --body-file ./runbook-v2.html --select templateId,name
```

### `confluence template delete <ID>`

Permanently deletes a template. Requires `--confirm` — unlike page delete, this is not a soft delete; there's no trash for templates.

```sh
cargo run -p confluence -- template delete 4321 --confirm
```

Prints `{"deleted": true, "id": "4321"}` on success.

### `--select <PATHS>` (global flag)

Same client-side field-projection flag as every other crate in this workspace — see root `CLAUDE.md`'s "Shared library: crates/cli-fields". Mandatory on every command in this crate except `doctor`, `auth login`, `auth whoami`, `auth logout`, `page delete`, `template delete` and `space get` (see this crate's `CLAUDE.md` for the full table) — omitting both `--select` and `--select-all` fails with the response's byte size and top-level field names instead of printing.

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
not logged in as the service account. Run: confluence auth login. To act as a person logged in with confluence auth login --user <USER_ID>, pass --user <USER_ID> instead
```

Errors are typed with `thiserror` (`CliError` in `error.rs`). Internal module errors (`LoginError`, `ClientError`) are mapped to `CliError` at the top-level `run()` function and never surface directly to the user.

### Exit codes

- `0` — success.
- `3` — the selected identity needs a new login: nothing stored for it, or its credentials file holds the other identity's login (`not logged in ...` / `user <id> is not logged in ...`), or the person's refresh token was refused (`the login of user <id> is no longer valid ...`), including when the API answered `401` to a token revoked before it expired and renewing it failed. Run the login command the message names. A caller (e.g. an agent acting for many people) can rely on this code instead of the message text.
- `2` — invalid arguments (reported by clap).
- `1` — every other failure, including a renewal that failed for a transient reason (network, 429, 5xx: retry), a refused renewal of the Service Account (its grant is its client id/secret in app.json, so a new login would not help), an app refused by the token endpoint (`invalid_client`: fix app.json, a new login through the same app would fail too), a credentials file that exists but can't be read or is corrupted, a `401` that persists with a freshly renewed token, and `doctor` with a failing check.
