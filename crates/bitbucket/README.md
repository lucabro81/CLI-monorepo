# bitbucket

CLI for Bitbucket Cloud, designed to be driven by an LLM agent (output is JSON, errors are actionable). This README documents it for humans setting it up and maintaining it; new commands get documented here as they're added.

## Status

`init`, `doctor`, `auth login`/`auth whoami`, `repo get`, `repo list`, `repo create`, `repo delete`, `pr get`, `pr list`, `pr create`, `pr comment`, `pr approve`, `pr unapprove`, `pr decline`, `pr merge`, `branch list` implemented. See [CLAUDE.md](CLAUDE.md) for architecture and the planned command list.

## Setup

### 1. Create a Bitbucket OAuth consumer

In your Bitbucket workspace, go to **Settings → OAuth consumers → Add consumer**:

- **Name**: anything descriptive, e.g. `bitbucket-cli`
- **Callback URL**: leave empty — the `client_credentials` grant doesn't use it
- **Permissions**: grant whatever scopes the commands you intend to use need (e.g. Account Read, Repositories Read/Write, Pull requests Read/Write)

After saving, note down the consumer's **Key** (`client_id`) and **Secret** (`client_secret`).

> The token's identity is whichever account created the consumer. In production this should be a dedicated `bot@<domain>` account added as a workspace member, not a personal account.

### 2. Write the app credentials file

Create `$XDG_CONFIG_HOME/bitbucket-cli/app.json` (typically `~/.config/bitbucket-cli/app.json`):

```json
{
  "client_id": "your-consumer-key",
  "client_secret": "your-consumer-secret"
}
```

This file is static and hand-written — the CLI never modifies it. It's kept separate from `credentials.json` (the dynamic token store, see below) precisely so that automatic token writes never overwrite your app identity.

### 3. Log in

```sh
cargo run -p bitbucket -- auth login
```

This exchanges the consumer's Key/Secret for an access token via `client_credentials` — no browser, no human interaction. Run this once per machine; tokens are renewed automatically after that.

### `bitbucket init` does all of the above

`bitbucket init` walks through steps 1–3 interactively: it prints the consumer-creation instructions, prompts for the Key/Secret (or accepts `--client-id`/`--client-secret` flags for non-interactive use), writes `app.json`, runs `auth login`, and prints a `doctor` JSON report as final confirmation.

```sh
cargo run -p bitbucket -- init
cargo run -p bitbucket -- init --client-id <KEY> --client-secret <SECRET>
```

## How the OAuth flow works

Bitbucket Cloud's native OAuth `client_credentials` grant is used — not the unified `developer.atlassian.com` OAuth used by `jira` (that app has no Bitbucket API permission to grant), and not Repository/Workspace Access Tokens (Premium-only).

1. **Token request** — the CLI POSTs `grant_type=client_credentials` to `https://bitbucket.org/site/oauth2/access_token`, authenticated with HTTP Basic auth using `client_id`/`client_secret` from `app.json`. Receives an `access_token`, an expiry, and a `scopes` field listing the OAuth scopes granted to the consumer.
2. **Persisting credentials** — `access_token`, `expires_at`, and `scopes` are written to `credentials.json`.
3. **API calls** — `https://api.bitbucket.org/2.0/...`, with the workspace slug used directly in paths. No `cloud_id` resolution step like `jira`.

### Automatic renewal

Before each API call, the CLI checks whether the access token is expired (or about to expire within 60s). There is no `refresh_token` — the access token is short-lived and is simply re-requested via the same `client_credentials` exchange when expired, and `credentials.json` is overwritten with the new values.

## Usage

### `bitbucket init`

Interactive onboarding. See [Setup](#setup) above.

```sh
cargo run -p bitbucket -- init
cargo run -p bitbucket -- init --client-id <KEY> --client-secret <SECRET>
```

### `bitbucket doctor`

Runs four checks and prints a structured JSON report: `app_config` (app.json exists and is valid), `credentials` (tokens exist and are not expired, renewed if needed), `api` (live call to `/2.0/user` succeeds), `permissions` (the OAuth scopes granted to the consumer). Exits non-zero if any check fails.

```sh
cargo run -p bitbucket -- doctor
cargo run -p bitbucket -- doctor --select app_config.status,credentials.status,api.status,permissions
```

The `permissions` check reports `granted_scopes` as-is from the token response — `status` is `"ok"` if the list is non-empty, `"error"` only if it's empty (nothing will work). It's purely informational beyond that: which scopes a given command needs is documented per-command below, not enforced by `doctor`.

### `bitbucket auth login`

Stores credentials locally. Runs the non-interactive `client_credentials` flow — no browser, no human interaction.

```sh
cargo run -p bitbucket -- auth login
```

Run this once per machine, or again if `credentials.json` is lost or revoked.

### `bitbucket auth whoami`

Prints the currently authenticated identity as JSON. With `client_credentials`, this is the **workspace** itself (`type: "team"`), not a personal user.

```sh
cargo run -p bitbucket -- auth whoami
cargo run -p bitbucket -- auth whoami --select uuid,display_name
```

### `bitbucket repo get <workspace>/<repo_slug>`

Fetches a single repository and prints the full Bitbucket API response as pretty-printed JSON.

```sh
cargo run -p bitbucket -- repo get lucabrognaracode/my-repo
cargo run -p bitbucket -- repo get lucabrognaracode/my-repo --select description,language
```

Requires the `repository` (read) scope.

### `bitbucket repo list <workspace>`

Lists repositories in a workspace, paginated.

```sh
cargo run -p bitbucket -- repo list lucabrognaracode
cargo run -p bitbucket -- repo list lucabrognaracode --page 2
cargo run -p bitbucket -- repo list lucabrognaracode --select values.full_name
```

**Flags:**
- `--page <N>` — page number to fetch (Bitbucket pagination starts at 1)

Requires the `repository` (read) scope.

### `bitbucket repo create <workspace>/<repo_slug>`

Creates a new repository. `scm` is always `git`. All flags are optional.

```sh
cargo run -p bitbucket -- repo create lucabrognaracode/my-new-repo
cargo run -p bitbucket -- repo create lucabrognaracode/my-new-repo --description "My new repo" --private
cargo run -p bitbucket -- repo create lucabrognaracode/my-new-repo --project PROJ
```

**Flags:**
- `--description <TEXT>` — repository description
- `--private` — create as a private repository (default: workspace default)
- `--project <KEY>` — assign the repository to a project in the workspace

Requires the `repository:write` scope. Note: some workspaces reject public repositories under a private project (`"Private projects cannot contain public repositories"`) — pass `--private` in that case.

### `bitbucket repo delete <workspace>/<repo_slug>`

Deletes a repository. **Destructive**: permanent and cannot be undone — requires `--confirm`.

```sh
cargo run -p bitbucket -- repo delete lucabrognaracode/my-repo --confirm
```

Returns `{"deleted": true, "repository": "<workspace>/<repo_slug>"}`. Requires the `repository:admin` scope.

### `bitbucket pr create <workspace>/<repo_slug>`

Creates a new pull request.

```sh
cargo run -p bitbucket -- pr create lucabrognaracode/my-repo --title "My PR" --source feature-branch
cargo run -p bitbucket -- pr create lucabrognaracode/my-repo --title "My PR" --source feature-branch --destination main --description "does things"
cargo run -p bitbucket -- pr create lucabrognaracode/my-repo --title "My PR" --source feature-branch --close-source-branch
```

**Flags:**
- `--title <TEXT>` — pull request title (required)
- `--source <BRANCH>` — source branch name (required)
- `--destination <BRANCH>` — destination branch name. If omitted, Bitbucket uses the repository's main branch.
- `--description <TEXT>` — pull request description
- `--close-source-branch` — close the source branch after the pull request is merged

Requires the `pullrequest:write` scope. Reviewers are not yet supported (see CLAUDE.md backlog).

### `bitbucket pr approve <workspace>/<repo_slug> <id>`

Approves a pull request as the authenticated account.

```sh
cargo run -p bitbucket -- pr approve lucabrognaracode/my-repo 42
```

Requires the `pullrequest:write` scope.

### `bitbucket pr unapprove <workspace>/<repo_slug> <id>`

Removes the authenticated account's approval from a pull request.

```sh
cargo run -p bitbucket -- pr unapprove lucabrognaracode/my-repo 42
```

Requires the `pullrequest:write` scope.

### `bitbucket pr decline <workspace>/<repo_slug> <id>`

Declines a pull request. **Destructive**: changes the pull request's state and cannot be undone by this CLI — requires `--confirm`.

```sh
cargo run -p bitbucket -- pr decline lucabrognaracode/my-repo 42 --confirm
```

Requires the `pullrequest:write` scope.

### `bitbucket pr merge <workspace>/<repo_slug> <id>`

Merges a pull request. **Destructive**: permanent and cannot be undone — requires `--confirm`.

```sh
cargo run -p bitbucket -- pr merge lucabrognaracode/my-repo 42 --confirm
cargo run -p bitbucket -- pr merge lucabrognaracode/my-repo 42 --merge-strategy squash --close-source-branch --confirm
```

**Flags:**
- `--message <TEXT>` — custom merge commit message. If omitted, Bitbucket generates a default.
- `--merge-strategy <STRATEGY>` — `merge_commit`, `squash`, or `fast_forward`. If omitted, Bitbucket uses the repository's default.
- `--close-source-branch` — close the source branch after merging

Requires the `pullrequest:write` scope.

### `bitbucket pr comment <workspace>/<repo_slug> <id>`

Adds a comment to a pull request — general or inline (attached to a file/line).

```sh
cargo run -p bitbucket -- pr comment lucabrognaracode/my-repo 42 --content "Looks good to me"
cargo run -p bitbucket -- pr comment lucabrognaracode/my-repo 42 --content "Fix this" --path src/main.rs --line 10
```

**Flags:**
- `--content <TEXT>` — comment text, Markdown (required)
- `--path <PATH>` and `--line <N>` — attach the comment to a line in a file (the new version's line number). Both or neither must be set.

Requires the `pullrequest:write` scope.

### `bitbucket pr get <workspace>/<repo_slug> <id>`

Fetches a single pull request and prints the full Bitbucket API response as pretty-printed JSON.

```sh
cargo run -p bitbucket -- pr get lucabrognaracode/my-repo 42
cargo run -p bitbucket -- pr get lucabrognaracode/my-repo 42 --select title,state,source.branch.name
```

Requires the `pullrequest` (read) scope.

### `bitbucket pr list <workspace>/<repo_slug>`

Lists pull requests in a repository, paginated.

```sh
cargo run -p bitbucket -- pr list lucabrognaracode/my-repo
cargo run -p bitbucket -- pr list lucabrognaracode/my-repo --state MERGED
cargo run -p bitbucket -- pr list lucabrognaracode/my-repo --page 2
cargo run -p bitbucket -- pr list lucabrognaracode/my-repo --select values.title,values.state
```

**Flags:**
- `--state <STATE>` — filter by `OPEN`, `MERGED`, `DECLINED`, or `SUPERSEDED`. If omitted, Bitbucket returns pull requests in any state.
- `--page <N>` — page number to fetch (Bitbucket pagination starts at 1)

Requires the `pullrequest` (read) scope.

### `bitbucket branch list <workspace>/<repo_slug>`

Lists branches in a repository, paginated.

```sh
cargo run -p bitbucket -- branch list lucabrognaracode/my-repo
cargo run -p bitbucket -- branch list lucabrognaracode/my-repo --page 2
cargo run -p bitbucket -- branch list lucabrognaracode/my-repo --select values.name
```

**Flags:**
- `--page <N>` — page number to fetch (Bitbucket pagination starts at 1)

Requires the `repository` (read) scope.

### `--select <PATHS>` (global flag)

All commands that return JSON support a `--select` flag for client-side field projection. Pass a comma-separated list of dot-notation paths; only those paths are included in the output. If omitted, the full response from Bitbucket is printed.

```sh
# only the fields you care about from a repo
cargo run -p bitbucket -- repo get lucabrognaracode/my-repo --select description,language,is_private

# just the full names from a repo list
cargo run -p bitbucket -- repo list lucabrognaracode --select values.full_name

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
not authenticated. Run: bitbucket auth login
```

Errors are typed with `thiserror` (`CliError` in `error.rs`). Internal module errors (`ClientError`, `OAuthConfigError`) are mapped to `CliError` at the top-level `run()` function and never surface directly to the user.

## Development

```sh
cargo build -p bitbucket
cargo run -p bitbucket -- --help
```
