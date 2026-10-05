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

Opens the browser on `<instance>/oauth/v2/authorize` for the Native app, waits for the redirect on `http://localhost:8080/callback`, and exchanges the code (with the PKCE verifier) for an access token and a refresh token. Every action is then attributed to the human.

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

### `zitadel auth login`

Logs in as the service user from `app.json` (private key JWT, see [above](#service-user-login-default-private-key-jwt)) and saves `credentials.json`. Normally only needed once: tokens are renewed automatically.

```sh
zitadel auth login
```

### `zitadel auth whoami`

The identity behind the stored credentials (`GET /auth/v1/users/me`): `id`, `userName`, `loginNames`, `details.resourceOwner` (its organization), and a `machine` (service user) or `human` object. Always printed in full (an explicit `--select` is honored).

```sh
zitadel auth whoami
zitadel auth whoami --select user.id,user.userName,user.details.resourceOwner
```

### `--select <PATHS>` / `--select-all` (global flags)

List/search commands require `--select` (comma-separated dot paths) or `--select-all`; without either they fail and report the response size and top-level fields. See root `CLAUDE.md`.

## Testing

```sh
cargo test -p zitadel                 # unit tests
cargo test -p zitadel -- --ignored    # e2e, needs `zitadel init` done and ZITADEL_E2E_* in .env
```

## Error design

Every error is a single plain-text sentence: what went wrong and what to run or change to fix it. In particular:

- missing/invalid `app.json` → the exact `zitadel init ...` command to run;
- `401` from the API → `zitadel auth login`;
- `403` from the API → the identity lacks an administrator role for that operation; run `zitadel doctor` to see its roles and grant the missing one in the console.
