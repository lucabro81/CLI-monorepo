# zitadel

CLI for the [ZITADEL](https://zitadel.com) identity platform (Cloud or self-hosted), designed to be driven by an LLM agent. Output is JSON; errors are plain text explaining what went wrong and how to fix it.

## Table of contents

- [Setup](#setup)
- [How the OAuth flow works](#how-the-oauth-flow-works)
- [Usage](#usage)
- [Testing](#testing)
- [Error design](#error-design)

## Setup

### 1. Service user (default identity, for agents)

1. In the ZITADEL console of your instance (`https://<instance>.zitadel.cloud` or your own domain): **Users → Service Users → New**.
2. Grant it an administrator role matching what the CLI should be able to do — e.g. **IAM_OWNER** (whole instance) via *Default settings → Administrators*, or **ORG_OWNER** / **ORG_USER_MANAGER** on an organization. The CLI can only do what this role allows; `zitadel doctor` lists the roles it sees.
3. On the service user: **Keys → New → JSON**, and download the key file. It contains a private key — keep it out of any repository.

### 2. (Optional) Native app, for people (`--user <USER_ID>`)

Only needed if commands must act as people (run with `--user <USER_ID>`); one Native app serves every person:

1. **Projects → Create** (or pick an existing project) → **New Application → Native**.
2. Authentication method: **PKCE**. Redirect URI: `http://localhost:8080/callback` (enable *Development Mode*, required for an `http` redirect) and, for people logging in from elsewhere (`auth login --user <USER_ID> --remote --redirect-uri <url>`), the HTTPS URL that receives the code, e.g. `https://<your server>/login/callback`.
3. In the token settings enable **Refresh Token**.
4. Note the **Client ID**.

### 3. Initialize

```sh
zitadel init --instance-url https://<instance>.zitadel.cloud --key-file ~/path/to/key.json
```

The key file's content is copied into `app.json` (mode `0600`); the original file is no longer needed by the CLI afterwards. For people, add the Native app and log the first one in through the browser (`jane.doe` is your own id for that person):

```sh
zitadel init --user jane.doe --client-id <CLIENT_ID>
```

Further people log in with `zitadel auth login --user <USER_ID>`.

See [`zitadel init`](#zitadel-init) below.

### The service user and any number of people, side by side

The CLI holds them all at once, and every command picks one per call: without `--user` it acts as the **service user**, with `--user <USER_ID>` as **that person**. `<USER_ID>` is your own name for the person, a lowercase slug (`a-z`, `0-9`, `.`, `_`, `-`, `:`, e.g. `jane.doe` or `chat:u123`). Each identity has its own credentials file — `credentials-service.json` (written by `zitadel auth login`) and `users/<USER_ID>/credentials.json` (written by `zitadel auth login --user <USER_ID>`) — so logging in as one never logs another out, and switching needs no new login while the tokens can be renewed:

```sh
zitadel user search --select result.userId                   # as the service user
zitadel user search --select result.userId --user jane.doe   # as jane.doe
```

`zitadel auth logout [--user <USER_ID>]` removes one identity's login. Leftover credentials files of earlier layouts (`credentials.json`, `credentials-user.json`) are ignored and reported by `zitadel doctor` so they can be deleted.

## How the OAuth flow works

### Service user login (default): private key JWT

`zitadel auth login` signs a short-lived JWT with the service user's private key (`RS256`, `kid` = key id, `iss`/`sub` = user id, `aud` = instance URL) and exchanges it at `<instance>/oauth/v2/token` using the `urn:ietf:params:oauth:grant-type:jwt-bearer` grant, with scope `openid urn:zitadel:iam:org:project:id:zitadel:aud`. No browser, no refresh token: when the access token is about to expire, the CLI simply signs a new assertion.

### Human login: authorization code + PKCE — `auth login --user <USER_ID>`

Opens the browser on `<instance>/oauth/v2/authorize` for the Native app (scopes `openid profile email offline_access urn:zitadel:iam:org:project:id:zitadel:aud`, PKCE `S256`, random `state`), listens on `127.0.0.1:8080` for the redirect to `http://localhost:8080/callback` (stray requests such as `/favicon.ico` get a 404 and the listener keeps waiting), checks `state`, and exchanges the code plus PKCE verifier for an access token and a refresh token. Every action is then attributed to that person. If port 8080 is busy (often a previous, aborted login), the command fails immediately instead of opening the browser.

### Remote login, in two steps — `auth login --user <USER_ID> --remote`

For a person who is not at the CLI's machine (the CLI runs on a server, the person is in a chat or a web page). Nothing opens a browser or listens on a port; whoever runs the CLI carries the link to the person and the code back.

1. `auth login --user <USER_ID> --remote --redirect-uri <url>` stores a pending login for that person (`state`, PKCE verifier, redirect URI; `users/<USER_ID>/pending-login.json`, mode `0600`) and prints `{"authorize_url", "state", "expires_at"}`. The redirect URI must be registered on the Native app (it accepts several).
2. The person opens `authorize_url` and logs in; ZITADEL redirects them to `<url>?code=...&state=...`.
3. `auth login --user <USER_ID> --code <code> --state <state>` checks the state and the expiry, exchanges the code with the stored verifier and redirect URI, saves `users/<USER_ID>/credentials.json`, and prints what `auth whoami --user <USER_ID>` prints.

`--remote`, `--code` and `--state` always need `--user <USER_ID>` (before or after `auth login`). A pending login is valid for 10 minutes and its state is single-use (consumed even if ZITADEL then refuses the code). Each person has their own, so several people can be mid-login at once; a new step 1 for the same person replaces theirs. `doctor --user <USER_ID>` shows it under `pending_login`.

### Automatic renewal

Each login writes its own identity's file. Before each command, the selected identity's token is renewed if it expires within 60 seconds — via the refresh token for a person, by re-signing the JWT for the service user — under a lock on the file, so parallel commands for the same identity renew once and share the result.

Config lives in `$XDG_CONFIG_HOME/zitadel-cli/` (fallback `~/.config/zitadel-cli/`): `app.json` (instance URL, service user key, optional client id), `credentials-service.json` and `users/<USER_ID>/credentials.json` (managed by the CLI), all with mode `0600`. An empty `<credentials file>.lock` (mode `0600`) appears next to a credentials file after its first renewal: it keeps parallel commands from renewing the same token twice. Leave it in place.

## Usage

Every command prints JSON on stdout and a single plain-text error on stderr (non-zero exit) on failure. Every command accepts the global `--user <USER_ID>` flag: without it the command acts as the service user, with it as that person.

### `zitadel init`

Like `jira init` / `bitbucket init`: prints numbered console setup steps, asks for whatever the flags didn't give, writes `app.json` (mode `0600`), logs in and prints the [`doctor`](#zitadel-doctor) report for that identity.

- `zitadel init` — the service user: asks the instance URL (only when `app.json` has none) and the service user's **whole key JSON**, pasted as downloaded (it is one line; hidden, no echo, on a terminal), validated like `--key-file`; then logs in as the service user.
- `zitadel init --user <USER_ID>` — the Native app every person uses: asks the instance URL (only when missing) and the Native app client id, then runs the browser login for that person.
- `zitadel init --user-app` — the same Native app setup, logging nobody in (see the flag below).

A flag skips its question; an empty answer writes nothing (exit 1, naming the flag); when stdin is piped every answer is one plain line (so the key JSON must be the single-line file as downloaded; `--key-file` takes any file). As with `jira init`, the key (or, for people, the client id) is asked on every run unless its flag is given; the instance URL and everything else stored are kept. Setup steps and questions go to stderr. Changing the instance URL (`--instance-url`) removes every stored login (the service user's and every person's).

| Flag | Description |
|---|---|
| `--instance-url <URL>` | Instance base URL (Cloud or self-hosted). Asked when `app.json` has none; this flag is the way to change it. Trailing `/` is stripped. |
| `--key-file <PATH>` | Service user JSON key from the console. Validated (JSON, `"type": "serviceaccount"`, RSA PEM) before anything is written. Without it, `zitadel init` asks for the key JSON (hidden on a terminal). With `--user` / `--user-app` it isn't asked, but a given file is still validated and stored. |
| `--client-id <ID>` | Native app client id, only needed for people (`--user <USER_ID>`), shared by all of them. Asked by `init --user <USER_ID>` / `init --user-app` when omitted. |
| `--user-app` | Log nobody in (not even the service user) and print only `{"app_config": ...}` instead of the doctor report: sets up what people log in with where nobody can open a browser; they log in later with `zitadel auth login --user <USER_ID> --remote --redirect-uri <URL>` (a hint saying so goes to stderr). Exits 1 if no Native app client id is configured. Takes no `--user`. |

```sh
zitadel init                                                        # asks the instance URL and the key JSON
zitadel init --instance-url https://acme.zitadel.cloud --key-file ~/Downloads/123456789.json
zitadel init --user jane.doe --client-id 123456789@zitadel-cli    # later: add the Native app and log jane.doe in
zitadel init --user-app --instance-url https://acme.zitadel.cloud --client-id 123456789@zitadel-cli   # Native app only, nobody logged in
```

Progress lines, setup steps and questions go to stderr; stdout carries only the doctor report (with `--user-app`, the `app_config` check). Exits non-zero if any doctor check fails.

### `zitadel doctor`

Four cascading checks for the selected identity (the service user, or the person with `--user <USER_ID>`), each with `"status": "ok" | "error" | "skipped"`:

- `app_config` — `app.json` valid; `instance_url`, `service_user_configured`, `native_app_configured`.
- `credentials` — that identity's stored token usable (renewed if expiring); `identity` is `service_user` or `user`, plus `expires_at`.
- `api` — `GET /auth/v1/users/me`: `user_id`, `user_name`, `type` (`machine`/`human`), `organization_id`.
- `memberships` — the identity's administrator roles, one entry per `{level, id, display_name, roles}` with `level` in `instance` / `organization` / `project` / `project_grant`. ZITADEL authorizes by these roles, so they decide which commands succeed; no membership at all is an error.

Two informational keys never affect the exit code: `pending_login` (the selected person's remote login) and `identities` (which identity was checked, whether the service user is logged in, the ids of the people logged in under `users`, and credentials files of earlier layouts under `legacy_credentials_files`). Always prints the full report (an explicit `--select` is honored). Exits non-zero unless every check is `ok`.

```sh
zitadel doctor
zitadel doctor --user jane.doe
zitadel doctor --select memberships
```

### `zitadel auth login [--user <USER_ID>]`

Logs in one identity and saves its credentials file, leaving every other's untouched. Normally only needed once per identity: tokens are renewed automatically.

- default — the service user from `app.json` (private key JWT, see [above](#service-user-login-default-private-key-jwt)), into `credentials-service.json`. No browser: the mode for agents.
- `--user <USER_ID>` — that person, through the browser (see [above](#human-login-authorization-code--pkce--auth-login---user-user_id)), into `users/<USER_ID>/credentials.json`. Needs the Native app's client id in `app.json` (`zitadel init --user <USER_ID> --client-id <id>`).
- `--user <USER_ID> --remote --redirect-uri <url>`, then `--user <USER_ID> --code <code> --state <state>` — someone who is not at this machine, in two steps (see [above](#remote-login-in-two-steps--auth-login---user-user_id---remote)). Step 1 prints `{authorize_url, state, expires_at}`; step 2 prints the `auth whoami --user <USER_ID>` output.

```sh
zitadel auth login          # service user
zitadel auth login --user jane.doe   # jane.doe, via the browser
zitadel auth login --user jane.doe --remote --redirect-uri https://app.example.com/oauth/callback   # step 1
zitadel auth login --user jane.doe --code <CODE> --state <STATE>                                   # step 2
```

### `zitadel auth logout [--user <USER_ID>]`

Removes the stored login of the service user, or of the person with `--user <USER_ID>` (their whole `users/<USER_ID>/` folder), and prints `{"logged_out": "service"}` or `{"logged_out": "user:<USER_ID>"}`. Other identities and `app.json` are untouched; tokens are not revoked at ZITADEL. Fails, naming the login command, if that identity had no stored login. Always printed in full.

```sh
zitadel auth logout --user jane.doe
```

### `zitadel auth whoami`

The account the CLI acts as (`GET /auth/v1/users/me`) — the service user, or the person with `--user <USER_ID>`: `id`, `userName`, `loginNames`, `details.resourceOwner` (its organization), and a `machine` (service user) or `human` object. Always printed in full (an explicit `--select` is honored).

```sh
zitadel auth whoami
zitadel auth whoami --user jane.doe
zitadel auth whoami --select user.id,user.userName,user.details.resourceOwner
```

### `zitadel user search`

Searches users with ZITADEL's v2 `ListUsers` (`POST /v2/users`). Filters are optional and combined with AND. Requires `--select` or `--select-all`. Needs the `user.read` permission (e.g. `ORG_OWNER` / `ORG_USER_MANAGER` on the users' organization) — otherwise only what the identity may read is returned.

| Flag | Description |
|---|---|
| `--email <TEXT>` | Email contains this text, case-insensitive (`@acme.com`, or a full address). |
| `--username <TEXT>` | Username contains this text, case-insensitive. |
| `--state <STATE>` | `active`, `inactive`, `deleted`, `locked`, `initial`. Validated locally: ZITADEL silently returns nothing for an unknown state. |
| `--organization-id <ID>` | Only users of this organization. |
| `--limit <N>` | Max results, default 100. |
| `--offset <N>` | Results to skip, default 0. |

Response: `details.totalResult` (total matches; absent, together with `result`, when there are none) and `result[]` with `userId`, `username`, `state`, `loginNames`, `details.resourceOwner` (organization) and a `human` (`profile`, `email`, `phone`) or `machine` object.

```sh
zitadel user search --email @acme.com --select result.userId,result.username,result.human.email.email
zitadel user search --username john --state active --select result.userId,result.state
zitadel user search --limit 50 --offset 50 --select details.totalResult,result.userId
```

### `zitadel user get <user-id>`

One user by id (v2 `GetUserByID`, `GET /v2/users/{userId}`): `details` plus `user` with the same fields as a `user search` result. Always printed in full (an explicit `--select` is honored). An unknown id fails with a 404 error suggesting `user search`.

```sh
zitadel user get 123456789012345678
zitadel user get 123456789012345678 --select user.username,user.state,user.human.email.email
```

### `zitadel organization list`

Organizations the identity may read (v2 `ListOrganizations`, `POST /v2/organizations/_search`). An instance administrator (`IAM_OWNER`) sees all of them, an `ORG_OWNER` only its own. Requires `--select` or `--select-all`.

| Flag | Description |
|---|---|
| `--name <TEXT>` | Name contains this text, case-insensitive. |
| `--limit <N>` | Max results, default 100. |
| `--offset <N>` | Results to skip, default 0. |

Response: `details.totalResult` and `result[]` with `id`, `name`, `state`, `primaryDomain` (both absent when nothing matches).

```sh
zitadel organization list --select result.id,result.name,result.state
zitadel organization list --name acme --select result.id,result.name
```

### `zitadel project list`

Projects the identity may read (v2 `ListProjects`, `POST /zitadel.project.v2.ProjectService/ListProjects`). Requires the `project.read` permission (e.g. `ORG_OWNER`, `PROJECT_OWNER`) and `--select` or `--select-all`.

| Flag | Description |
|---|---|
| `--name <TEXT>` | Name contains this text, case-insensitive. |
| `--organization-id <ID>` | Only projects owned by this organization. |
| `--limit <N>` | Max results, default 100. |
| `--offset <N>` | Results to skip, default 0. |

Response (note the different shape from users/organizations): `pagination.totalResult` (absent when nothing matches), `pagination.appliedLimit`, and `projects[]` with `projectId`, `name`, `state`, `organizationId`, `creationDate`, `changeDate`.

```sh
zitadel project list --select projects.projectId,projects.name
zitadel project list --organization-id 123456789 --name app --select pagination.totalResult,projects.projectId
```

### `--select <PATHS>` / `--select-all` (global flags)

List/search commands require `--select` (comma-separated dot paths) or `--select-all`; without either they fail and report the response size and top-level fields. See root `CLAUDE.md`.

## Testing

```sh
cargo test -p zitadel                 # unit tests
cargo test -p zitadel -- --ignored    # e2e against the configured instance (needs `zitadel init`)
```

The e2e suite is read-only: it uses the logged-in identity itself (its id, username and organization) as the known fixture, so it creates nothing.

## Error design

Every error is a single plain-text sentence: what went wrong and what to run or change to fix it. In particular:

- missing/invalid `app.json` → the exact `zitadel init ...` command to run;
- `401` from the API → `zitadel auth login`;
- a failed `auth login --user <USER_ID>` → what to check on the Native app (client id, redirect URI, PKCE, refresh token);
- `403` from the API → the identity lacks an administrator role for that operation; run `zitadel doctor` to see its roles and grant the missing one in the console.

### Exit codes

- `0` — success.
- `3` — the selected identity needs a new login: nothing stored for it, or its credentials file holds the other identity's login, or is corrupted (`not logged in ...` / `user <id> is not logged in ...`), or the person's refresh token was refused (`the login of user <id> is no longer valid ...`). Run the login command the message names. A caller (e.g. an agent acting for many people) can rely on this code instead of the message text.
- `2` — invalid arguments (reported by clap).
- `1` — every other failure, including a renewal that failed for a transient reason (network, 429, 5xx: retry), a refused renewal of the service user (its grant is its key in app.json, so a new login would not help), an app refused by the token endpoint (`invalid_client`: fix app.json, a new login through the same app would fail too), a credentials file that exists but can't be read, a `401` from the API with a stored token (the message still names `zitadel auth login`), and `doctor` with a failing check.
