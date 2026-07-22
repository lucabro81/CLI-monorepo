# atlassian-admin

CLI for the Atlassian Organization Admin API, designed to be driven by an LLM agent (output is JSON, errors are actionable). This README documents it for humans setting it up and maintaining it; new commands get documented here as they're added.

## Table of contents

- [Status](#status)
- [Setup](#setup)
- [How auth works](#how-auth-works)
- [Usage](#usage)
  - [`atlassian-admin init`](#atlassian-admin-init)
  - [`atlassian-admin doctor`](#atlassian-admin-doctor)
  - [`atlassian-admin user get`](#atlassian-admin-user-get)
  - [`--select <PATHS>` (global flag)](#--select-paths-global-flag)
- [Testing](#testing)
- [Error design](#error-design)

## Status

`init`, `doctor`, `user get` implemented. See [CLAUDE.md](CLAUDE.md) for architecture.

## Setup

### 1. Create an Organization API key

You need to be an admin (or superadmin) of your Atlassian organization.

1. Go to [admin.atlassian.com](https://admin.atlassian.com), select your organization.
2. **Organization settings → API keys**.
3. Choose **API keys with scopes** (recommended over "without scopes").
4. **Create API key** — give it a descriptive name and an expiration (max 1 year; you'll need to rotate it before then).
5. Select scopes: `read:accounts:admin` and `read:directories:admin` cover every command this CLI currently implements.
6. **Create** — the confirmation screen shows the **Organization ID** and the **API key together, once**. Copy both immediately; they cannot be recovered afterward.

This key is org-wide and significantly more privileged than a single-product OAuth consumer — treat it accordingly (never commit it, prefer the narrowest scope set your commands need).

### 2. Write the app credentials file

Create `$XDG_CONFIG_HOME/atlassian-admin-cli/app.json` (typically `~/.config/atlassian-admin-cli/app.json`):

```json
{
  "api_key": "your-organization-api-key",
  "org_id": "your-organization-id"
}
```

This file is static — the CLI never modifies it. There is no separate credentials/token file: this API key is a finished, long-lived credential, not something exchanged for a short-lived access token.

### `atlassian-admin init` does the above for you

```sh
cargo run -p atlassian-admin -- init --api-key <KEY> --org-id <ORG_ID>
```

Unlike other crates in this workspace, `init` does **not** fall back to an interactive stdin prompt if you omit the flags — pasting an org-wide secret into a terminal prompt risks it landing in scrollback or session logs. Instead:

```sh
cargo run -p atlassian-admin -- init
```

creates `app.json` as an empty skeleton (if it doesn't already exist) and prints the exact path — open it in an editor and paste your key/org ID in by hand, then re-run `doctor` to verify.

## How auth works

No OAuth grant, no consumer, no client_id/secret pair, no token expiry or refresh. Every request sends the API key from `app.json` directly as `Authorization: Bearer <api_key>` to `https://api.atlassian.com/admin/v1/orgs/{org_id}/...`.

The Organization Admin API only resolves accounts that are **managed** under your organization — i.e. their email domain is verified/claimed via Atlassian Access/Guard. An external Atlassian account (personal ID, unrelated domain) won't resolve here regardless of scope.

## Usage

### `atlassian-admin init`

Interactive-free onboarding. See [Setup](#setup) above.

```sh
cargo run -p atlassian-admin -- init
cargo run -p atlassian-admin -- init --api-key <KEY> --org-id <ORG_ID>
```

### `atlassian-admin doctor`

Runs two checks and prints a structured JSON report: `app_config` (app.json exists and is well-formed), `api` (live call to `GET /v1/orgs/{org_id}` succeeds). Exits non-zero if any check fails.

```sh
cargo run -p atlassian-admin -- doctor
cargo run -p atlassian-admin -- doctor --select app_config.status,api.status
```

### `atlassian-admin user get`

Resolves an Atlassian `account_id` — the same identity shared across Jira, Confluence, and Bitbucket since the 2019 account unification — to a full profile (including email), for managed accounts only.

```sh
cargo run -p atlassian-admin -- user get --account-id 712020:b6d01943-f1de-4eb4-ab1a-300a17283d42
cargo run -p atlassian-admin -- user get --account-id 712020:b6d01943-f1de-4eb4-ab1a-300a17283d42 --select email,name
```

Requires the `read:accounts:admin` + `read:directories:admin` scopes.

### `--select <PATHS>` (global flag)

All commands that return JSON support a `--select` flag for client-side field projection. Pass a comma-separated list of dot-notation paths; only those paths are included in the output. If omitted, the full response is printed.

```sh
cargo run -p atlassian-admin -- user get --account-id <ID> --select email
```

The flag can appear before or after the subcommand.

## Testing

### Unit tests

No external dependencies. Run with:

```sh
cargo test -p atlassian-admin
```

### Live testing

New commands are smoke-tested manually against a real organization during development:

```sh
cargo run -p atlassian-admin -- <command> --help     # accurate, complete help text?
cargo run -p atlassian-admin -- <command> ...         # against a real organization
```

No automated e2e suite yet — see `crates/atlassian-admin/CLAUDE.md`.

## Error design

All errors are plain text, no colors or symbols — designed to be read by an LLM. Each message is self-contained: it states what went wrong and what to do next.

Errors are typed with `thiserror` (`CliError` in `error.rs`). Internal module errors are mapped to `CliError` at the top-level `run()` function and never surface directly to the user.
