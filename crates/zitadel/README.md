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

### 2. (Optional) Native app, for `auth login --user`

Only needed if a human wants to use the CLI as themselves:

1. **Projects → Create** (or pick an existing project) → **New Application → Native**.
2. Authentication method: **PKCE**. Redirect URI: `http://localhost:8080/callback` (enable *Development Mode*, required for an `http` redirect).
3. In the token settings enable **Refresh Token**.
4. Note the **Client ID**.

### 3. Initialize

```sh
zitadel init --instance-url https://<instance>.zitadel.cloud --key-file ~/path/to/key.json
```

The key file's content is copied into `app.json` (mode `0600`); the original file is no longer needed by the CLI afterwards. See [`zitadel init`](#zitadel-init) below.

## How the OAuth flow works

### Service user login (default): private key JWT

`zitadel auth login` signs a short-lived JWT with the service user's private key (`RS256`, `kid` = key id, `iss`/`sub` = user id, `aud` = instance URL) and exchanges it at `<instance>/oauth/v2/token` using the `urn:ietf:params:oauth:grant-type:jwt-bearer` grant, with scope `openid urn:zitadel:iam:org:project:id:zitadel:aud`. No browser, no refresh token: when the access token is about to expire, the CLI simply signs a new assertion.

### Human login: authorization code + PKCE — `auth login --user`

Opens the browser on `<instance>/oauth/v2/authorize` for the Native app (scopes `openid profile email offline_access urn:zitadel:iam:org:project:id:zitadel:aud`, PKCE `S256`, random `state`), listens on `127.0.0.1:8080` for the redirect to `http://localhost:8080/callback` (stray requests such as `/favicon.ico` get a 404 and the listener keeps waiting), checks `state`, and exchanges the code plus PKCE verifier for an access token and a refresh token. Every action is then attributed to the human. If port 8080 is busy (often a previous, aborted login), the command fails immediately instead of opening the browser.

### Remote login, in two steps — `auth login --user --remote`

For a person who is not at the CLI's machine (the CLI runs on a server, the person is in a chat or a web page). Nothing opens a browser or listens on a port; whoever runs the CLI carries the link to the person and the code back.

1. `auth login --user --remote --redirect-uri <url>` stores a pending login (`state`, PKCE verifier, redirect URI; `pending-login.json`, mode `0600`) and prints `{"authorize_url", "state", "expires_at"}`. The redirect URI must be registered on the Native app (it accepts several).
2. The person opens `authorize_url` and logs in; ZITADEL redirects them to `<url>?code=...&state=...`.
3. `auth login --user --code <code> --state <state>` checks the state and the expiry, exchanges the code with the stored verifier and redirect URI, saves `credentials.json`, and prints what `auth whoami` prints.

A pending login is valid for 10 minutes and its state is single-use (consumed even if ZITADEL then refuses the code). A new step 1 replaces the previous pending login. Everything lives in the config folder the CLI resolves, so pointing `XDG_CONFIG_HOME` at one folder per person keeps people's logins apart. `doctor` shows a pending login under `pending_login`.

### Automatic renewal

Both logins write `credentials.json`; the last one decides the identity. Before each command, a token expiring within 60 seconds is renewed — via the refresh token for `--user`, by re-signing the JWT for the service user.

Config lives in `$XDG_CONFIG_HOME/zitadel-cli/` (fallback `~/.config/zitadel-cli/`): `app.json` (instance URL, service user key, optional client id) and `credentials.json` (managed by the CLI).

## Usage

Every command prints JSON on stdout and a single plain-text error on stderr (non-zero exit) on failure.

### `zitadel init`

Writes `app.json`, logs in as the service user (if a key is configured) and prints the [`doctor`](#zitadel-doctor) report. Flag-driven, no prompts. Re-running merges with the existing `app.json`: omitted flags keep their value.

| Flag | Description |
|---|---|
| `--instance-url <URL>` | Instance base URL (Cloud or self-hosted). Required on the first run. Trailing `/` is stripped. |
| `--key-file <PATH>` | Service user JSON key from the console. Validated (JSON, `"type": "serviceaccount"`, RSA PEM) before anything is written. |
| `--client-id <ID>` | Native app client id, only needed for `auth login --user`. |

```sh
zitadel init --instance-url https://acme.zitadel.cloud --key-file ~/Downloads/123456789.json
zitadel init --client-id 123456789@zitadel-cli    # later: add the Native app, keep everything else
```

Progress lines go to stderr; stdout carries only the doctor report. Exits non-zero if any doctor check fails.

### `zitadel doctor`

Four cascading checks, each with `"status": "ok" | "error" | "skipped"`:

- `app_config` — `app.json` valid; `instance_url`, `service_user_configured`, `native_app_configured`.
- `credentials` — stored token usable (renewed if expiring); `identity` is `service_user` or `user`, plus `expires_at`.
- `api` — `GET /auth/v1/users/me`: `user_id`, `user_name`, `type` (`machine`/`human`), `organization_id`.
- `memberships` — the identity's administrator roles, one entry per `{level, id, display_name, roles}` with `level` in `instance` / `organization` / `project` / `project_grant`. ZITADEL authorizes by these roles, so they decide which commands succeed; no membership at all is an error.

Always prints the full report (an explicit `--select` is honored). Exits non-zero unless every check is `ok`.

```sh
zitadel doctor
zitadel doctor --select memberships
```

### `zitadel auth login [--user]`

Saves `credentials.json` for one of two identities; the last login decides which one the CLI acts as. Normally only needed once: tokens are renewed automatically.

- default — the service user from `app.json` (private key JWT, see [above](#service-user-login-default-private-key-jwt)). No browser: the mode for agents.
- `--user` — yourself, through the browser (see [above](#human-login-authorization-code--pkce--auth-login---user)). Needs the Native app's client id in `app.json` (`zitadel init --client-id <id>`).
- `--user --remote --redirect-uri <url>`, then `--user --code <code> --state <state>` — someone who is not at this machine, in two steps (see [above](#remote-login-in-two-steps--auth-login---user---remote)). Step 1 prints `{authorize_url, state, expires_at}`; step 2 prints the `auth whoami` output.

```sh
zitadel auth login          # service user
zitadel auth login --user   # human, via the browser
zitadel auth login --user --remote --redirect-uri https://app.example.com/oauth/callback   # step 1
zitadel auth login --user --code <CODE> --state <STATE>                                   # step 2
```

### `zitadel auth whoami`

The identity behind the stored credentials (`GET /auth/v1/users/me`): `id`, `userName`, `loginNames`, `details.resourceOwner` (its organization), and a `machine` (service user) or `human` object. Always printed in full (an explicit `--select` is honored).

```sh
zitadel auth whoami
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
- a failed `auth login --user` → what to check on the Native app (client id, redirect URI, PKCE, refresh token);
- `403` from the API → the identity lacks an administrator role for that operation; run `zitadel doctor` to see its roles and grant the missing one in the console.
