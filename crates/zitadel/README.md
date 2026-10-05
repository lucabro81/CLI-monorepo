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
zitadel init --instance-url https://<instance>.zitadel.cloud --key-file ~/path/to/key.json [--client-id <native-app-client-id>]
```

TODO: final flags/behaviour documented once `init` is implemented.

## How the OAuth flow works

### Service user login (default): private key JWT

`zitadel auth login` signs a short-lived JWT with the service user's private key (`RS256`, `kid` = key id, `iss`/`sub` = user id, `aud` = instance URL) and exchanges it at `<instance>/oauth/v2/token` using the `urn:ietf:params:oauth:grant-type:jwt-bearer` grant, with scope `openid urn:zitadel:iam:org:project:id:zitadel:aud`. No browser, no refresh token: when the access token is about to expire, the CLI simply signs a new assertion.

### Human login: authorization code + PKCE — `auth login --user`

Opens the browser on `<instance>/oauth/v2/authorize` for the Native app, waits for the redirect on `http://localhost:8080/callback`, and exchanges the code (with the PKCE verifier) for an access token and a refresh token. Every action is then attributed to the human.

### Automatic renewal

Both logins write `credentials.json`; the last one decides the identity. Before each command, a token expiring within 60 seconds is renewed — via the refresh token for `--user`, by re-signing the JWT for the service user.

Config lives in `$XDG_CONFIG_HOME/zitadel-cli/` (fallback `~/.config/zitadel-cli/`): `app.json` (instance URL, service user key, optional client id) and `credentials.json` (managed by the CLI).

## Usage

TODO — one section per command as they are implemented: `init`, `doctor`, `auth login`, `auth whoami`, `user search`, `user get`, `organization list`, `project list`.

### `--select <PATHS>` / `--select-all` (global flags)

List/search commands require `--select` (comma-separated dot paths) or `--select-all`; without either they fail and report the response size and top-level fields. See root `CLAUDE.md`.

## Testing

```sh
cargo test -p zitadel                 # unit tests
cargo test -p zitadel -- --ignored    # e2e, needs `zitadel init` done and ZITADEL_E2E_* in .env
```

## Error design

Every error is a single plain-text sentence: what went wrong and what to run or change to fix it (e.g. a missing `app.json` points to `zitadel init`, a 403 points to the role the identity is missing).
