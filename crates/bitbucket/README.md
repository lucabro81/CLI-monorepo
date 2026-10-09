# bitbucket

CLI for Bitbucket Cloud, designed to be driven by an LLM agent (output is JSON, errors are actionable). This README documents it for humans setting it up and maintaining it; new commands get documented here as they're added.

## Table of contents

- [Status](#status)
- [Setup](#setup)
- [How the OAuth flow works](#how-the-oauth-flow-works)
- [Usage](#usage)
  - [`bitbucket init`](#bitbucket-init)
  - [`bitbucket doctor`](#bitbucket-doctor)
  - [`bitbucket auth login`](#bitbucket-auth-login)
  - [`bitbucket auth logout`](#bitbucket-auth-logout)
  - [`bitbucket auth whoami`](#bitbucket-auth-whoami)
  - [`bitbucket repo get <workspace>/<repo_slug>`](#bitbucket-repo-get-workspacerepo_slug)
  - [`bitbucket repo list <workspace>`](#bitbucket-repo-list-workspace)
  - [`bitbucket repo create <workspace>/<repo_slug>`](#bitbucket-repo-create-workspacerepo_slug)
  - [`bitbucket repo delete <workspace>/<repo_slug>`](#bitbucket-repo-delete-workspacerepo_slug)
  - [`bitbucket pr create <workspace>/<repo_slug>`](#bitbucket-pr-create-workspacerepo_slug)
  - [`bitbucket pr update <workspace>/<repo_slug> <id>`](#bitbucket-pr-update-workspacerepo_slug-id)
  - [`bitbucket pr approve <workspace>/<repo_slug> <id>`](#bitbucket-pr-approve-workspacerepo_slug-id)
  - [`bitbucket pr unapprove <workspace>/<repo_slug> <id>`](#bitbucket-pr-unapprove-workspacerepo_slug-id)
  - [`bitbucket pr decline <workspace>/<repo_slug> <id>`](#bitbucket-pr-decline-workspacerepo_slug-id)
  - [`bitbucket pr merge <workspace>/<repo_slug> <id>`](#bitbucket-pr-merge-workspacerepo_slug-id)
  - [`bitbucket pr diff <workspace>/<repo_slug> <id>`](#bitbucket-pr-diff-workspacerepo_slug-id)
  - [`bitbucket pr comment <workspace>/<repo_slug> <id>`](#bitbucket-pr-comment-workspacerepo_slug-id)
  - [`bitbucket pr list-comments <workspace>/<repo_slug> <id>`](#bitbucket-pr-list-comments-workspacerepo_slug-id)
  - [`bitbucket pr update-comment <workspace>/<repo_slug> <id> <comment_id>`](#bitbucket-pr-update-comment-workspacerepo_slug-id-comment_id)
  - [`bitbucket pr get <workspace>/<repo_slug> <id>`](#bitbucket-pr-get-workspacerepo_slug-id)
  - [`bitbucket pr list <workspace>/<repo_slug>`](#bitbucket-pr-list-workspacerepo_slug)
  - [`bitbucket branch list <workspace>/<repo_slug>`](#bitbucket-branch-list-workspacerepo_slug)
  - [`bitbucket branch create <workspace>/<repo_slug> <name>`](#bitbucket-branch-create-workspacerepo_slug-name)
  - [`bitbucket branch suggest-name --issue-key <KEY> --issue-type <TYPE> --issue-summary <SUMMARY>`](#bitbucket-branch-suggest-name---issue-key-key---issue-type-type---issue-summary-summary)
  - [`bitbucket workspace members <workspace>`](#bitbucket-workspace-members-workspace)
  - [`--select <PATHS>` / `--select-all` (global flags)](#--select-paths----select-all-global-flags)
- [Testing](#testing)
- [Error design](#error-design)

## Status

`init`, `doctor`, `auth login`/`auth whoami`/`auth logout`, `repo get`, `repo list`, `repo create`, `repo delete`, `pr get`, `pr list`, `pr create`, `pr update`, `pr comment`, `pr list-comments`, `pr update-comment`, `pr approve`, `pr unapprove`, `pr decline`, `pr merge`, `pr diff`, `branch list`, `branch create`, `branch suggest-name`, `workspace members` implemented. See [CLAUDE.md](CLAUDE.md) for architecture and the planned command list.

## Setup

### 1. Create a Bitbucket OAuth client

In your Bitbucket workspace, go to **Settings → Apps and features → OAuth clients → Create OAuth client**:

- **Name**: anything descriptive, e.g. `bitbucket-cli`
- **Callback URL**: `http://localhost:8080/callback` — needed only for `auth login --user <USER_ID>`; the default `client_credentials` login doesn't use it
- **Permissions**: grant whatever scopes the commands you intend to use need (e.g. Account Read, Repositories Read/Write, Pull requests Read/Write)

After saving, note down the consumer's **Key** (`client_id`) and **Secret** (`client_secret`).

### 2. Save it, log in and verify

The CLI holds the OAuth app and any number of people side by side, and every command picks one per call: without `--user` it acts as the **OAuth app** (bot identity, `client_credentials`), with `--user <USER_ID>` as **that person** (who approved the consent page when logging in). `<USER_ID>` is your own name for the person, a lowercase slug (`a-z`, `0-9`, `.`, `_`, `-`, `:`, e.g. `jane.doe` or `chat:u123`). All of them live in `$XDG_CONFIG_HOME/bitbucket-cli/` (typically `~/.config/bitbucket-cli/`), every file with mode `0600`:

| File | Written by | Holds |
|---|---|---|
| `app.json` | `bitbucket init` / `bitbucket init --user <USER_ID>` / `bitbucket init --user-app` | `"service"` section (the consumer used as the app) and `"user"` section (the consumer every person logs in through) — either may be missing; both may hold the same consumer |
| `credentials-service.json` | `bitbucket auth login` | the app's token |
| `users/<USER_ID>/credentials.json` | `bitbucket auth login --user <USER_ID>` | that person's token and refresh token |
| `users/<USER_ID>/pending-login.json` | `bitbucket auth login --user <USER_ID> --remote` | that person's remote login waiting for its code |

An empty `<credentials file>.lock` (mode `0600`) appears next to a credentials file after its first renewal: it keeps parallel commands from renewing the same token twice. Leave it in place.

```sh
cargo run -p bitbucket -- init --client-id <KEY> --client-secret <SECRET>          # the OAuth app, no browser
cargo run -p bitbucket -- init --user jane.doe --client-id <KEY> --client-secret <SECRET>   # jane.doe, via browser consent
cargo run -p bitbucket -- init --user-app --client-id <KEY> --client-secret <SECRET>   # the people's consumer only, nobody logged in
```

`init` prints the consumer-creation instructions, prompts for the Key/Secret when the flags are omitted (the Secret hidden, no echo, on a terminal; a plain line when stdin is piped; an empty answer writes nothing), writes that identity's section of `app.json` (leaving the other alone), logs in as that identity, and prints a `doctor` JSON report for it. `app.json` is static — the CLI never modifies it at runtime — and kept separate from the credentials files so automatic token writes never overwrite your app identity. `init --user-app` writes only the `"user"` consumer and logs nobody in (e.g. in a container nobody sits at): it prints only `{"app_config": ...}` for that section, then the commands that log people in (`bitbucket auth login --user <USER_ID>`, or `--remote` for a person elsewhere); it takes no `--user` and exits 1 if the section can't be read back. Each login touches only its own credentials file; further people log in with `auth login --user <USER_ID>`, and `auth logout [--user <USER_ID>]` removes one identity's login. An `app.json` in the old flat format (`client_id` at top level, before issue #164) is rejected with the commands to recreate it; leftover credentials files of earlier layouts (`credentials.json`, `credentials-user.json`) are ignored and reported by `doctor`.

Day-to-day, no identity needs a new login: tokens are renewed automatically. An agent picks the identity on every call:

```sh
cargo run -p bitbucket -- repo get <workspace>/<repo_slug>                   # as the OAuth app
cargo run -p bitbucket -- repo get <workspace>/<repo_slug> --user jane.doe   # as jane.doe
```

## How the OAuth flow works

Bitbucket Cloud's native OAuth `client_credentials` grant is used — not the unified `developer.atlassian.com` OAuth used by `jira` (that app has no Bitbucket API permission to grant), and not Repository/Workspace Access Tokens (Premium-only).

1. **Token request** — the CLI POSTs `grant_type=client_credentials` to `https://bitbucket.org/site/oauth2/access_token`, authenticated with HTTP Basic auth using `client_id`/`client_secret` from `app.json`'s `"service"` section. Receives an `access_token`, an expiry, and a `scopes` field listing the OAuth scopes granted to the consumer.
2. **Persisting credentials** — `access_token`, `expires_at`, and `scopes` are written to `credentials-service.json`.
3. **API calls** — `https://api.bitbucket.org/2.0/...`, with the workspace slug used directly in paths. No `cloud_id` resolution step like `jira`.

### Automatic renewal

Before each API call, the CLI checks whether the selected identity's access token is expired (or about to expire within 60s). The app's credentials (`credentials-service.json`) have no `refresh_token`: the token is re-requested via the same `client_credentials` exchange. A person's (`users/<USER_ID>/credentials.json`) use the stored `refresh_token` (Bitbucket rotates it on every use; an unused one expires after 3 months, then run `auth login --user <USER_ID>` again). Either way only that identity's file is overwritten with the new values, under a lock on the file: parallel commands for the same identity renew once and share the result.

**A token revoked before it expires** (e.g. the person ended their session or revoked the app's access; issue #240) only shows up as a `401` from the API. The CLI then renews that identity's token once, under the same lock, and repeats the request (safe: a `401` means the request was not processed). If that renewal is refused, a person's command exits `3` (see [Exit codes](#exit-codes)); a `401` that persists with the fresh token is reported as is.

## Usage

Every command accepts the global `--user <USER_ID>` flag: without it the command acts as the OAuth app, with it as that person (see [Setup](#setup)).

### `bitbucket init`

Onboarding for one identity. See [Setup](#setup) above.

```sh
cargo run -p bitbucket -- init --client-id <KEY> --client-secret <SECRET>
cargo run -p bitbucket -- init --user jane.doe --client-id <KEY> --client-secret <SECRET>
cargo run -p bitbucket -- init --user-app --client-id <KEY> --client-secret <SECRET>
```

### `bitbucket doctor`

Runs four checks for the selected identity (the OAuth app, or the person with `--user <USER_ID>`) and prints a structured JSON report: `app_config` (app.json has the identity's section), `credentials` (that identity's tokens exist and are not expired, renewed if needed), `api` (live call to `/2.0/user` succeeds), `permissions` (the OAuth scopes granted to the consumer). A token the `api` call gets a `401` for is renewed and the call repeated, as every command does; if that renewal fails, `credentials` reports the error (with the login command) and the later checks are `skipped`. Exits non-zero if any check fails. Two informational keys never affect the exit code: `pending_login` (the selected person's remote login) and `identities` (which identity was checked, whether the OAuth app is logged in, the ids of the people logged in under `users`, and credentials files of earlier layouts under `legacy_credentials_files`).

```sh
cargo run -p bitbucket -- doctor
cargo run -p bitbucket -- doctor --user jane.doe
cargo run -p bitbucket -- doctor --select app_config.status,credentials.status,api.status,permissions,identities
```

The `permissions` check reports `granted_scopes` as-is from the token response — `status` is `"ok"` if the list is non-empty, `"error"` only if it's empty (nothing will work). It's purely informational beyond that: which scopes a given command needs is documented per-command below, not enforced by `doctor`.

### `bitbucket auth login`

Logs in one identity and stores its credentials, leaving every other identity's untouched:

- default — non-interactive `client_credentials` flow with the `"service"` consumer, no browser, saved to `credentials-service.json`. Commands run without `--user` are attributed to the **OAuth app** (bot identity): the mode for agents.
- `--user <USER_ID>` — interactive `authorization_code` flow with the `"user"` consumer, saved to `users/<USER_ID>/credentials.json`: opens the browser on Bitbucket's consent page, receives the callback on `localhost:8080`, stores a refresh token. Commands run with `--user <USER_ID>` are attributed to **that person's account**. Requires the consumer's callback URL to be `http://localhost:8080/callback`.
- `--user <USER_ID> --remote`, then `--user <USER_ID> --code <code> --state <state>` — the same `authorization_code` flow in two steps, for a person who is not at the CLI's machine (the CLI on a server, the person in a chat or a web page). Step 1 opens no browser and listens on no port: it stores a pending login for that person (`state` only; `users/<USER_ID>/pending-login.json`, mode `0600`, so several people can be mid-login at once) and prints `{"authorize_url", "state", "expires_at"}`. The person opens `authorize_url` and grants access; Bitbucket redirects them to the consumer's callback URL with `code` and `state`. Step 2 checks the state and the expiry, exchanges the code, saves `users/<USER_ID>/credentials.json`, and prints what `auth whoami --user <USER_ID>` prints. A pending login is valid for 10 minutes and its state is single-use. `--remote`, `--code` and `--state` always need `--user <USER_ID>` (before or after `auth login`).

```sh
cargo run -p bitbucket -- auth login
cargo run -p bitbucket -- auth login --user jane.doe
cargo run -p bitbucket -- auth login --user jane.doe --remote                          # step 1
cargo run -p bitbucket -- auth login --user jane.doe --code <CODE> --state <STATE>     # step 2
```

**Bitbucket has no `redirect_uri` parameter**: the person is always sent back to the consumer's single callback URL. So for remote logins through a service (e.g. one that receives the redirect at `https://service.example.com/oauth/callback`), the `"user"` section must hold a consumer whose callback URL is that endpoint; a config folder (`XDG_CONFIG_HOME`) whose `"user"` consumer calls back to `localhost:8080` is the one for local `--user` logins. `doctor --user <USER_ID>` shows that person's pending login under `pending_login`.

Run each once per identity per machine, or again if that identity's credentials file is lost or revoked. `doctor` reports the checked credentials' kind as `credentials.identity` (`app` or `user`).

### `bitbucket auth logout`

Removes the stored login of the OAuth app, or of the person with `--user <USER_ID>` (their whole `users/<USER_ID>/` folder), and prints `{"logged_out": "service"}` or `{"logged_out": "user:<USER_ID>"}`. Other identities and `app.json` are untouched; tokens are not revoked at Bitbucket. Fails, naming the login command, if that identity had no stored login.

```sh
cargo run -p bitbucket -- auth logout --user jane.doe
```

### `bitbucket auth whoami`

Prints the account the CLI acts as, as JSON: without `--user` the OAuth app's identity (not a personal user), with `--user <USER_ID>` that person's account.

```sh
cargo run -p bitbucket -- auth whoami
cargo run -p bitbucket -- auth whoami --user jane.doe
cargo run -p bitbucket -- auth whoami --select uuid,display_name
```

### `bitbucket repo get <workspace>/<repo_slug>`

Fetches a single repository and prints the full Bitbucket API response as pretty-printed JSON.

```sh
cargo run -p bitbucket -- repo get <workspace>/my-repo
cargo run -p bitbucket -- repo get <workspace>/my-repo --select description,language
```

Requires the `repository` (read) scope.

### `bitbucket repo list <workspace>`

Lists repositories in a workspace, paginated.

```sh
cargo run -p bitbucket -- repo list <workspace> --select values.full_name
cargo run -p bitbucket -- repo list <workspace> --page 2 --select values.full_name
cargo run -p bitbucket -- repo list <workspace> --select-all
```

**Flags:**
- `--page <N>` — page number to fetch (Bitbucket pagination starts at 1)

Requires the `repository` (read) scope.

### `bitbucket repo create <workspace>/<repo_slug>`

Creates a new repository. `scm` is always `git`. All flags are optional.

```sh
cargo run -p bitbucket -- repo create <workspace>/my-new-repo
cargo run -p bitbucket -- repo create <workspace>/my-new-repo --description "My new repo" --private
cargo run -p bitbucket -- repo create <workspace>/my-new-repo --project PROJ
```

**Flags:**
- `--description <TEXT>` — repository description
- `--private` — create as a private repository (default: workspace default)
- `--project <KEY>` — assign the repository to a project in the workspace

Requires the `repository:write` scope. Note: some workspaces reject public repositories under a private project (`"Private projects cannot contain public repositories"`) — pass `--private` in that case.

### `bitbucket repo delete <workspace>/<repo_slug>`

Deletes a repository. **Destructive**: permanent and cannot be undone — requires `--confirm`.

```sh
cargo run -p bitbucket -- repo delete <workspace>/my-repo --confirm
```

Returns `{"deleted": true, "repository": "<workspace>/<repo_slug>"}`. Requires the `repository:admin` scope.

### `bitbucket pr create <workspace>/<repo_slug>`

Creates a new pull request.

```sh
cargo run -p bitbucket -- pr create <workspace>/my-repo --title "My PR" --source feature-branch
cargo run -p bitbucket -- pr create <workspace>/my-repo --title "My PR" --source feature-branch --destination main --description "does things"
cargo run -p bitbucket -- pr create <workspace>/my-repo --title "My PR" --source feature-branch --close-source-branch
cargo run -p bitbucket -- pr create <workspace>/my-repo --title "My PR" --source feature-branch --reviewers "{xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx}"
cargo run -p bitbucket -- pr create <workspace>/my-repo --title "WIP: My PR" --source feature-branch --draft
```

**Flags:**
- `--title <TEXT>` — pull request title (required)
- `--source <BRANCH>` — source branch name (required)
- `--destination <BRANCH>` — destination branch name. If omitted, Bitbucket uses the repository's main branch.
- `--description <TEXT>` — pull request description
- `--close-source-branch` — close the source branch after the pull request is merged
- `--reviewers <UUIDS>` — comma-separated reviewer UUIDs, each in curly braces (e.g. `{xxxxxxxx-...}`); find them with `bitbucket workspace members <workspace>`
- `--draft` — create the pull request as a draft; publish it later with `pr update --ready-for-review`

Requires the `pullrequest:write` scope.

### `bitbucket pr update <workspace>/<repo_slug> <id>`

Updates an open pull request's title, description, destination branch, reviewers, or draft status. Only the fields you pass are changed, with one exception: `--reviewers` replaces the entire reviewer list rather than adding to it.

```sh
cargo run -p bitbucket -- pr update <workspace>/my-repo 42 --title "New title"
cargo run -p bitbucket -- pr update <workspace>/my-repo 42 --description "Updated description"
cargo run -p bitbucket -- pr update <workspace>/my-repo 42 --destination develop
cargo run -p bitbucket -- pr update <workspace>/my-repo 42 --reviewers "{xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx}"
cargo run -p bitbucket -- pr update <workspace>/my-repo 42 --draft
cargo run -p bitbucket -- pr update <workspace>/my-repo 42 --ready-for-review
```

**Flags** (at least one required):
- `--title <TEXT>` — new pull request title
- `--description <TEXT>` — new pull request description
- `--destination <BRANCH>` — new destination branch name
- `--reviewers <UUIDS>` — comma-separated reviewer UUIDs, same format as `pr create --reviewers`; replaces the full reviewer list
- `--draft` — convert the pull request to a draft (mutually exclusive with `--ready-for-review`)
- `--ready-for-review` — mark a draft pull request as ready for review (mutually exclusive with `--draft`)

Requires the `pullrequest:write` scope.

### `bitbucket pr approve <workspace>/<repo_slug> <id>`

Approves a pull request as the authenticated account.

```sh
cargo run -p bitbucket -- pr approve <workspace>/my-repo 42
```

Requires the `pullrequest:write` scope.

### `bitbucket pr unapprove <workspace>/<repo_slug> <id>`

Removes the authenticated account's approval from a pull request.

```sh
cargo run -p bitbucket -- pr unapprove <workspace>/my-repo 42
```

Requires the `pullrequest:write` scope.

### `bitbucket pr decline <workspace>/<repo_slug> <id>`

Declines a pull request. **Destructive**: changes the pull request's state and cannot be undone by this CLI — requires `--confirm`.

```sh
cargo run -p bitbucket -- pr decline <workspace>/my-repo 42 --confirm
```

Requires the `pullrequest:write` scope.

### `bitbucket pr merge <workspace>/<repo_slug> <id>`

Merges a pull request. **Destructive**: permanent and cannot be undone — requires `--confirm`.

```sh
cargo run -p bitbucket -- pr merge <workspace>/my-repo 42 --confirm
cargo run -p bitbucket -- pr merge <workspace>/my-repo 42 --merge-strategy squash --close-source-branch --confirm
```

**Flags:**
- `--message <TEXT>` — custom merge commit message. If omitted, Bitbucket generates a default.
- `--merge-strategy <STRATEGY>` — `merge_commit`, `squash`, or `fast_forward`. If omitted, Bitbucket uses the repository's default.
- `--close-source-branch` — close the source branch after merging

Requires the `pullrequest:write` scope.

### `bitbucket pr diff <workspace>/<repo_slug> <id>`

Prints the raw unified diff for a pull request as plain text (not JSON — `--select` has no effect).

```sh
cargo run -p bitbucket -- pr diff <workspace>/my-repo 42
cargo run -p bitbucket -- pr diff <workspace>/my-repo 42 --context 5
cargo run -p bitbucket -- pr diff <workspace>/my-repo 42 --path src/main.rs
```

**Flags:**
- `--context <N>` — number of unchanged context lines around each change. If omitted, Bitbucket uses its default.
- `--path <PATH>` — restrict the diff to a single file path.

Requires the `pullrequest` (read) scope.

### `bitbucket pr comment <workspace>/<repo_slug> <id>`

Adds a comment to a pull request — general, inline (attached to a file/line), or a reply to an existing comment.

```sh
cargo run -p bitbucket -- pr comment <workspace>/my-repo 42 --content "Looks good to me"
cargo run -p bitbucket -- pr comment <workspace>/my-repo 42 --content "Fix this" --path src/main.rs --line 10
cargo run -p bitbucket -- pr comment <workspace>/my-repo 42 --content "Done, fixed" --parent 123456
```

**Flags:**
- `--content <TEXT>` — comment text, Markdown (required)
- `--path <PATH>` and `--line <N>` — attach the comment to a line in a file (the new version's line number). Both or neither must be set.
- `--parent <COMMENT_ID>` — reply to an existing comment (IDs from `pr list-comments`). A reply to an inline comment stays on the parent's file and line, so `--parent` cannot be combined with `--path`/`--line`.

Requires the `pullrequest:write` scope.

### `bitbucket pr list-comments <workspace>/<repo_slug> <id>`

Lists every comment on a pull request, oldest first: general comments, inline comments and replies. Deleted comments are included too, marked with `deleted: true`, so no filtering happens on the client side. Replies carry `parent.id`, and inline comments carry `inline.path` and `inline.to`. `--select` is required because the list is paginated and unbounded.

```sh
cargo run -p bitbucket -- pr list-comments <workspace>/my-repo 42 --select values.id,values.content.raw,values.user.display_name,values.deleted,values.inline.path
cargo run -p bitbucket -- pr list-comments <workspace>/my-repo 42 --page 2 --select values.id,values.content.raw
```

**Flags:**
- `--page <N>` — page to fetch (Bitbucket pagination starts at 1)

Requires the `pullrequest` (read) scope.

### `bitbucket pr update-comment <workspace>/<repo_slug> <id> <comment_id>`

Replaces the text of an existing pull request comment. Only the text changes: an inline comment keeps its file and line. Bitbucket normally lets only the comment's author edit it. Use `pr list-comments` to find comment IDs.

```sh
cargo run -p bitbucket -- pr update-comment <workspace>/my-repo 42 123456 --content "Updated: looks good to me"
```

**Flags:**
- `--content <TEXT>` — new comment text, Markdown (required). It replaces the existing text entirely.

Requires the `pullrequest:write` scope.

### `bitbucket pr get <workspace>/<repo_slug> <id>`

Fetches a single pull request and prints the full Bitbucket API response as pretty-printed JSON.

```sh
cargo run -p bitbucket -- pr get <workspace>/my-repo 42
cargo run -p bitbucket -- pr get <workspace>/my-repo 42 --select title,state,source.branch.name
```

Requires the `pullrequest` (read) scope.

### `bitbucket pr list <workspace>/<repo_slug>`

Lists pull requests in a repository, paginated.

```sh
cargo run -p bitbucket -- pr list <workspace>/my-repo --select values.id,values.title,values.state
cargo run -p bitbucket -- pr list <workspace>/my-repo --state MERGED --select values.id,values.title
cargo run -p bitbucket -- pr list <workspace>/my-repo --page 2 --select values.id,values.title
cargo run -p bitbucket -- pr list <workspace>/my-repo --select-all
```

**Flags:**
- `--state <STATE>` — filter by `OPEN`, `MERGED`, `DECLINED`, or `SUPERSEDED`. If omitted, Bitbucket returns pull requests in any state.
- `--page <N>` — page number to fetch (Bitbucket pagination starts at 1)

Requires the `pullrequest` (read) scope.

### `bitbucket branch list <workspace>/<repo_slug>`

Lists branches in a repository, paginated.

```sh
cargo run -p bitbucket -- branch list <workspace>/my-repo --select values.name
cargo run -p bitbucket -- branch list <workspace>/my-repo --page 2 --select values.name
cargo run -p bitbucket -- branch list <workspace>/my-repo --select-all
```

**Flags:**
- `--page <N>` — page number to fetch (Bitbucket pagination starts at 1)

Requires the `repository` (read) scope.

### `bitbucket branch create <workspace>/<repo_slug> <name>`

Creates a new branch in a repository.

```sh
cargo run -p bitbucket -- branch create <workspace>/my-repo feature/my-branch --target main
```

**Flags:**
- `--target <TARGET>` — branch name or commit hash to create the new branch from (required)

Requires the `repository:write` scope.

### `bitbucket branch suggest-name --issue-key <KEY> --issue-type <TYPE> --issue-summary <SUMMARY>`

Computes a suggested Bitbucket branch name from a Jira issue's key/type/summary
(useful when composing a branch name for `branch create` from a `jira issue
create`/`jira issue get` result — see the root README for how the two CLIs
are meant to be composed by an agent). The branch-type prefix (`feature/`,
`bugfix/`, ...) is resolved in one of three ways, tried in order:

1. `--prefix <PREFIX>` — explicit override, used verbatim. No lookup, no network call.
2. `--repository <workspace>/<repo_slug>` — looks up that repository's actual
   configured branching model (`GET /2.0/repositories/{workspace}/{repo_slug}/branching-model`)
   for the real prefix. Requires authentication.
3. Neither given — falls back to a local best-effort heuristic (`Bug` → `bugfix`,
   anything else → `feature`), no network call, no auth needed. This is a guess,
   not guaranteed to match a specific repo's actual branching model — use
   `--repository` or `--prefix` for a reliable result.

The response's `prefix_source` field (`"override"` / `"branching_model"` /
`"heuristic"`) reports which of the three produced the prefix.

```sh
# offline heuristic
cargo run -p bitbucket -- branch suggest-name --issue-key SBF-19 --issue-type Task --issue-summary "Costruire griglia Smartlocker v2"

# real lookup against a repo's branching model
cargo run -p bitbucket -- branch suggest-name --issue-key SBF-19 --issue-type Task --issue-summary "..." --repository <workspace>/my-repo

# explicit override
cargo run -p bitbucket -- branch suggest-name --issue-key SBF-19 --issue-type Task --issue-summary "..." --prefix hotfix
```

**Flags:**
- `--repository <workspace/repo_slug>` — optional; triggers the real branching-model lookup
- `--prefix <PREFIX>` — optional; explicit override, skips inference and any lookup

No auth/scope required unless `--repository` is used, in which case it requires the `repository` (read) scope (same as `branch list`).

### `bitbucket workspace members <workspace>`

Lists the members of a workspace, paginated (`GET /2.0/workspaces/{workspace}/members`). This is how to resolve a person to the `uuid` that `pr create --reviewers` and `pr update --reviewers` need: each entry's `user.uuid` (curly braces included) goes straight into `--reviewers`.

`--select` is mandatory (the response is a paginated collection): pass `--select` with the paths you need, or `--select-all` to print the whole page.

```sh
cargo run -p bitbucket -- workspace members <workspace> --select values.user.uuid,values.user.display_name
cargo run -p bitbucket -- workspace members <workspace> --page 2 --select values.user.uuid,values.user.display_name
cargo run -p bitbucket -- workspace members <workspace> --select-all
```

**Flags:**
- `--page <N>` — page number to fetch (Bitbucket pagination starts at 1)

Requires the `account` scope.

### `--select <PATHS>` / `--select-all` (global flags)

All commands that return JSON support a `--select` flag for client-side field projection. Pass a comma-separated list of dot-notation paths; only those paths are included in the output.

**`--select` is mandatory on list commands** (`repo list`, `pr list`, `pr list-comments`, `branch list`, `workspace members`): their responses are paginated collections that can be large. Omitting both `--select` and `--select-all` fails with an error giving the response's byte size and top-level field names, instead of printing it. `--select-all` is the explicit opt-out that prints the whole response, but a response over 30000 bytes (pretty-printed) is still refused, so narrow it with `--select` instead.

Every other JSON command (`doctor`, `auth whoami`, `auth logout`, `repo get`/`create`/`delete`, `pr get`/`create`/`update`/`comment`/`update-comment`/`approve`/`unapprove`/`decline`/`merge`, `branch create`, `branch suggest-name`) returns a single object that stays small. It prints in full when `--select` is omitted, and `--select` still narrows it. The same 30000-byte cap applies. `pr diff` prints raw diff text, not JSON, so `--select` has no effect on it.

```sh
# only the fields you care about from a repo
cargo run -p bitbucket -- repo get <workspace>/my-repo --select description,language,is_private

# just the full names from a repo list (mandatory: list command)
cargo run -p bitbucket -- repo list <workspace> --select values.full_name

# a whole page of a list, explicitly
cargo run -p bitbucket -- repo list <workspace> --select-all

# just your account details
cargo run -p bitbucket -- auth whoami --select uuid,display_name
```

The flag can appear before or after the subcommand. Arrays (like `values` in `repo list`) are projected element-wise automatically — no special syntax needed.

## Testing

### Unit tests

No external dependencies. Run with:

```sh
cargo test -p bitbucket
```

### End-to-end / live testing

An automated e2e suite exercises the full pr lifecycle against a real workspace
(see `crates/bitbucket/CLAUDE.md` for details). Requires `bitbucket auth login`
and `git` on `PATH`:

```sh
cargo test -p bitbucket -- --ignored --test-threads=1
```

New commands are also smoke-tested manually against a real workspace during development:

```sh
cargo run -p bitbucket -- <command> --help     # accurate, complete help text?
cargo run -p bitbucket -- <command> ...        # against a real workspace
```

## Error design

All errors are plain text, no colors or symbols — designed to be read by an LLM. Each message is self-contained: it states what went wrong and what to do next. Example:

```
not logged in as the OAuth app. Run: bitbucket auth login. To act as a person logged in with bitbucket auth login --user <USER_ID>, pass --user <USER_ID> instead
```

Errors are typed with `thiserror` (`CliError` in `error.rs`). Internal module errors (`ClientError`, `OAuthConfigError`) are mapped to `CliError` at the top-level `run()` function and never surface directly to the user.

### Exit codes

- `0` — success.
- `3` — the selected identity needs a new login: nothing stored for it, or its credentials file holds the other identity's login (`not logged in ...` / `user <id> is not logged in ...`), or the person's refresh token was refused (`the login of user <id> is no longer valid ...`), including when the API answered `401` to a token revoked before it expired and renewing it failed. Run the login command the message names. A caller (e.g. an agent acting for many people) can rely on this code instead of the message text.
- `2` — invalid arguments (reported by clap).
- `1` — every other failure, including a renewal that failed for a transient reason (network, 429, 5xx: retry), a refused renewal of the OAuth app (its grant is the consumer Key/Secret in app.json, so a new login would not help), an app refused by the token endpoint (`invalid_client`: fix app.json, a new login through the same app would fail too), a credentials file that exists but can't be read or is corrupted, a `401` that persists with a freshly renewed token, and `doctor` with a failing check.
