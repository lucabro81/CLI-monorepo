# CLAUDE.md — crates/jira

Architecture and design notes for the `jira` crate. Global rules (TDD, error handling, flag conventions, commands) are in the root `CLAUDE.md`.

## Module map

```
src/
  commands/
    mod.rs        — pub mod declarations for all command handlers
    auth.rs       — run_login(), run_whoami()
    doctor.rs     — run_doctor(); also called by init as final verification
    init.rs       — run_init(), write_app_config(); human onboarding flow
    issue.rs      — run(IssueCommand); dispatches all issue subcommands; also
                    holds comment-add's mention-building logic (see below)
    user.rs       — run(UserCommand); dispatches user subcommands (search)
    project.rs    — run(ProjectCommand); dispatches project subcommands (search)
  auth.rs         — thin wrapper over the shared `atlassian_auth` crate, fixing
                    this crate's config dir name and OAuth SCOPES: OAuthConfig,
                    Credentials, login(), login_client_credentials(), renew(),
                    save_credentials(), load_credentials(), path helpers,
                    get_granted_scopes(). The actual OAuth 2.0 (3LO + PKCE /
                    client_credentials) implementation, PKCE helpers, callback
                    parsing, and cloud_id resolution live in `atlassian_auth`
                    (workspace-local, shared with `confluence` — see root
                    CLAUDE.md's "Shared library: crates/atlassian-auth" and
                    BACKLOG.md's LIB-1)
  client.rs       — JiraClient (blocking reqwest); get_json/post_json helpers;
                    all Jira API methods: get_issue, get_myself, get_my_permissions,
                    add_comment (takes pre-built ADF content nodes), delete_comment,
                    get_transitions, list_transitions_json, apply_transition,
                    create_issue, delete_issue, assign_issue, search_issues,
                    search_users, get_user, search_projects
  cli.rs          — clap structs: Cli (--select global), Command, AuthCommand,
                    IssueCommand, CommentCommand, UserCommand, ProjectCommand. No logic.
  context.rs      — config_dir(), load_oauth_config(), authenticated_client(),
                    print_json(value, select), client_error_to_cli(e) (shared
                    ClientError -> CliError mapping used by every command handler
                    that calls JiraClient). Shared by all command handlers.
  endpoints.rs    — URL/path constants for Jira REST API v3, used by client.rs.
                    No logic. Atlassian OAuth endpoint constants live in
                    `atlassian_auth::endpoints` instead (shared with confluence).
  error.rs        — CliError (top-level, thiserror-derived). Includes IoError
                    for stdin prompts in init, and a transparent Select variant
                    wrapping cli_fields::RenderError.
  tests/          — all *_tests.rs files, mirroring the src/ layout (see "Test file
                    convention" below). tests/e2e_tests.rs holds the ignored e2e tests.
  main.rs         — pure dispatch: resolve --select/--select-all into a
                    cli_fields::Select once, match Command, call commands::*.
```

`--select` dot-notation projection itself (`filter_fields`, `describe_top_level_shape`, the `Select` enum, `render_json`) lives in the shared `crates/cli-fields` workspace crate, not in this crate — see root `CLAUDE.md`'s "Shared library: crates/cli-fields".

## Running tests

```sh
# Unit tests (no credentials needed)
cargo test -p jira

# E2e tests (requires login + a writable Jira project) — sequential, see README
JIRA_E2E_PROJECT=MER cargo test -p jira -- --ignored --test-threads=1

# Recovery: delete all [jira-cli-e2e] orphaned issues
JIRA_E2E_PROJECT=MER cargo test -p jira e2e_cleanup -- --ignored
```

`JIRA_E2E_PROJECT` can also be set once in a workspace-root `.env` (see
`.env.example`) instead of exporting it inline every run — `setup()` in
`src/tests/e2e_tests.rs` loads it via `dotenvy::dotenv()`, and an
already-exported value still takes precedence. See `BACKLOG.md`'s
`TESTENV-1` for why this exists.

## Test file convention

See root `CLAUDE.md` for the general `src/tests/` convention and the
cli_tests/commands split. `issue.rs` has a dedicated `tests/commands/issue_tests.rs`
covering `apply_stale_filter` (the `--stale-days` JQL builder),
`parse_body_segments` (the `{{mention:ACCOUNT_ID}}` placeholder parser used by
`comment add`), and `validate_assign_target` (the "exactly one of --assignee/
--unassign" runtime check for `issue assign` — `clap`'s `conflicts_with` only
rules out passing both, not passing neither) — the non-HTTP logic in the
module, besides `issue transition`'s case-insensitive status matching, which
stays covered end-to-end via `cli_tests.rs` instead. `user.rs` and
`project.rs` have no dedicated unit-test file: each has one command that's a
thin passthrough, covered entirely by `cli_tests.rs`.

## OAuth / auth design

**Implementation lives in `atlassian-auth`** (workspace-local crate, `crates/atlassian-auth`), not in this crate — `auth.rs` here is a thin wrapper fixing the `jira-cli` config dir name and this crate's `SCOPES` constant. `confluence` uses the exact same underlying flows (same `auth.atlassian.com`/`api.atlassian.com` endpoints, same `cloud_id` resolution), just with its own scopes — see `BACKLOG.md`'s `LIB-1` for why this was extracted (this crate's OAuth logic was about to be duplicated a third time, byte-for-byte, when `confluence` was added) and `atlassian-auth`'s own module docs for what it deliberately does *not* cover (`bitbucket`'s native OAuth consumer, `atlassian-admin`'s static API key — both genuinely different auth models, not more instances of this duplication).

Two grant types, both using `client_id`/`client_secret` from `app.json`:

- **`client_credentials`** (default, `auth login`) — `login_client_credentials()` POSTs `grant_type=client_credentials` + `audience=api.atlassian.com`, no browser. Returns `Credentials` with `refresh_token: None`. Expected mode for agent-driven usage; resulting account has `accountType: "app"`.
- **3LO + PKCE** (`auth login --user`, also used by `init`) — `login()` builds the authorization URL, opens the browser, runs a one-shot TCP server on `localhost:8080` for the callback, exchanges the code for tokens, resolves `cloud_id` via the accessible-resources endpoint. Returns `Credentials` with `refresh_token: Some(...)`.

### App identity sourcing: 3LO app vs. Service Account

`app.json`'s `client_id`/`client_secret` can come from either of two different Atlassian consoles, and this is what actually determines whether `client_credentials` needs a prior human step — not anything in this crate's code, which sends the identical request either way. Verified empirically against a real org: a Service Account's OAuth 2.0 credential works immediately with the existing `login_client_credentials()`/`fetch_cloud_id()` code, unmodified — `jira auth login` + `jira doctor` (all six checks) succeeded on the first try, no code change needed.

| Source | Console | Site access provisioning | Can also do 3LO (`auth login --user`)? |
|---|---|---|---|
| 3LO app | developer.atlassian.com/console/myapps | Human completes the 3LO consent screen once (`jira init` / `auth login --user`); until then `client_credentials` fails with "no accessible resources" | Yes — that's what issues the credential in the first place |
| Service Account | admin.atlassian.com → Directory → Service accounts → Create credentials → OAuth 2.0 | Assigned directly by an org admin at credential-creation time in the console; no consent screen exists or is needed | No — a Service Account's OAuth2 client only supports `client_credentials` |

`jira init` always ends with the 3LO browser flow, so it only makes sense for the 3LO app path. Service Account setup skips `init` entirely: write `app.json` by hand, then run `jira auth login` directly (see README Setup, Option A).

Both grants resolve `cloud_id` via the accessible-resources endpoint after obtaining the access token. `fetch_cloud_id` takes the first entry returned — this crate only supports a single Jira site per `app.json`/`credentials.json`. For the 3LO app path this requires the Atlassian app to be registered as **Resource-level** access type (not Account-level) in the developer console, so the 3LO consent screen limits the grant to one site (see README Setup, Option B). Supporting multiple sites (Account-level access, site selection) is a separate feature, not a config tweak.

- **Refresh tokens rotate**: Atlassian invalidates the previous refresh token on every use. The new token pair must be written to `credentials.json` immediately after each refresh.
- **Transparent renewal**: `renew(config, credentials)` dispatches to `refresh()` (if `refresh_token` is `Some`) or re-runs `login_client_credentials()` (if `None`, service account). `refresh()` itself returns `LoginError::Internal` if called with `refresh_token: None`. Both `load_credentials()` and `doctor`'s `check_credentials` go through `renew()` (with a 60s expiry buffer) — never call `refresh()` directly on possibly-expired credentials.
- **Scopes**: `read:jira-work read:jira-user write:jira-work offline_access` are what the 3LO authorization URL requests. `client_credentials` has no `scope` parameter in its own request body — it inherits whatever scopes were granted at credential-creation time: from the 3LO consent screen for a 3LO app, or from the scopes selected in admin.atlassian.com when the OAuth 2.0 credential was created for a Service Account.
- The `client_credentials` grant only requires the `--user` flow to have been completed at least once **when `app.json` holds a 3LO app's credentials**. When `app.json` holds Service Account credentials, `client_credentials` works immediately — site access was already assigned by an org admin in the console, not via a consent step this crate could observe or trigger.

## Config layout (XDG-style)

Both files live under `$XDG_CONFIG_HOME/jira-cli/` (falling back to `~/.config/jira-cli/`):

- `app.json` — `{"client_id": "...", "client_secret": "..."}`. Static; written by `jira init` (3LO app path only) or by hand (either path). Never modified at runtime. The shape is identical whether the credentials came from a 3LO app (developer.atlassian.com) or a Service Account (admin.atlassian.com) — see "OAuth / auth design" above for how the two differ in site-access provisioning.
- `credentials.json` — OAuth tokens. Fully managed by the CLI; never edit by hand.

Kept separate so automatic token writes never clobber the app identity.

## API design notes

- **Search endpoint**: `GET /rest/api/3/search/jql` (the old `POST /rest/api/3/search` is 410 Gone). Cursor-based pagination via `nextPageToken`.
- **`--select`/`--select-all`** (global flags, see root `CLAUDE.md`): `--select` is mandatory by default; omitting both flags fails with the response's byte size and top-level fields instead of printing. `--select-all` is the explicit stateless opt-out. Exempt commands (always print in full via `select.or_all()` at their `print_json` call site) and why:
  | Command | Exempt? | Why |
  |---|---|---|
  | `doctor` | yes | internally-generated report, fixed/small |
  | `auth whoami` | yes | identity check, fixed/small |
  | `issue get` | **no** | issues carry arbitrary per-project custom fields — can be large even for one record |
  | `issue search` | **no** | paginated list, same custom-field risk multiplied |
  | `issue create` | yes | `POST /issue` returns only `{id, key, self}` |
  | `issue delete` | yes | synthesized by us: `{"deleted": true, "key": ...}` |
  | `issue transitions` | yes | bounded workflow-state list, no `expand` requested |
  | `issue transition` | yes | synthesized by us: `{"transitioned": true, ...}` |
  | `issue assign` | yes | synthesized by us: `{"assigned": true, "key": ..., "assignee": ...}` — the underlying `PUT /issue/{key}/assignee` returns 204 No Content |
  | `issue comment add` | yes | single comment object, fixed shape |
  | `issue comment remove` | yes | synthesized by us: `{"deleted": true, "id": ...}` |
  | `user search` | **no** | list endpoint, same unbounded-response risk as `issue search` |
  | `project search` | **no** | list endpoint, same unbounded-response risk as `issue search`/`user search` — unbounded in principle even though this account currently sees only one project |
- **`--fields`** (issue search only): server-side Jira field selection. Defaults to `*navigable`. Reduces payload at the source; orthogonal to `--select`.
- **`--stale-days`** (issue search only): client-side JQL rewriting, not a separate API call — Jira's JQL grammar supports relative-date literals (`-Nd`) directly in a comparison (`updated <= -Nd`), evaluated server-side by Jira's own query engine. `apply_stale_filter` (`commands/issue.rs`) appends `AND updated <= -Nd` to `--jql`, inserting it immediately before an existing `ORDER BY` clause (found case-insensitively) since JQL requires `ORDER BY` to be the final clause — appending unconditionally would produce invalid JQL for any `--jql` that already sorts results.
- **ADF**: comment bodies and issue descriptions are wrapped in Atlassian Document Format by the client methods. `client::add_comment` takes pre-built ADF content nodes (`Vec<Value>`) rather than raw text, since `comment add` may need to mix plain `text` nodes with `mention` nodes — `commands/issue.rs` builds that list (`build_comment_content`, `parse_body_segments`, `build_text_node`, `build_mention_node`) before calling it.
- **Mentions in comments** (`issue comment add --mention <ACCOUNT_ID>` and/or `{{mention:ACCOUNT_ID}}` inside `--body`): both forms build an ADF `mention` node (`{"type": "mention", "attrs": {"id": ..., "text": "@..."}}`). The `text` attrs value is resolved to the user's real current display name via `client::get_user` (`GET /rest/api/3/user?accountId=...`) before the comment is posted — one extra API call per mention — rather than a generic placeholder, since Jira's fallback `text` is what's shown in contexts where the ADF isn't fully re-rendered (some notification digests). A lookup failure (e.g. unknown account ID) surfaces as the same `ApiError`/`ApiRequestFailed` as any other Jira API call — no special-cased error handling. `jira user search` is the way to discover an account ID for a name/email.
- **Destructive commands**: no interactive prompts (an LLM cannot respond). `issue delete` requires explicit `--confirm`; error message includes the exact command to retry. `commands/issue.rs::run` calls `authenticated_client()` **per match arm** rather than once up front, so the `--confirm` check (free, local) runs before the network round-trip a token refresh may require — otherwise a caller who forgot `--confirm` with expired credentials would see a confusing auth error instead of the actionable `DeleteNotConfirmed` one (spotted and fixed alongside `google-chat`'s `messages delete`, which had the same hoisted-auth structure).
- **Assignee endpoint**: `PUT /rest/api/3/issue/{key}/assignee` takes `{"accountId": "..."}` to assign, or `{"accountId": null}` to unassign — verified live against a real Jira Cloud site rather than trusted from docs, since Atlassian's own docs/community answers are ambiguous between this and the older Jira Server `{"name": null}` form. `issue assign`'s `--assignee`/`--unassign` flags are mutually exclusive (`clap`'s `conflicts_with`); `validate_assign_target` (`commands/issue.rs`) is the runtime check that at least one was passed, since `conflicts_with` alone doesn't enforce that.
- **`project search`**: `GET /rest/api/3/project/search?query=` is a literal substring/prefix filter (case-insensitive) against project key and name — not a query language, and distinct from JQL's `project = <value>` clause, which resolves a project's *exact, full* name or key but does not fragment-match (verified live: `project = Mercury` resolves to that project's issues, `project = mercur` returns zero). `client::search_projects` is a separate method from `client::list_projects` (used only internally by `doctor`'s `projects` check) even though both hit the same endpoint — `list_projects` paginates and discards everything but `key`, which is wrong for this command's purpose (surfacing `name` is the whole point); same reasoning as `list_transitions_json`/`get_transitions` being two separate views over one endpoint.

## `doctor` permission checks

Three independent layers, each surfaced as its own report key (see
[oauth-scopes-vs-permissions.md](docs/oauth-scopes-vs-permissions.md) for the
conceptual background):

- **`oauth_scopes`** — OAuth scopes granted to the token, from the
  accessible-resources endpoint (`auth::get_granted_scopes`). `error` if empty.
- **`service_user`** — `GET /mypermissions` with no `projectKey`: lists which
  of `PERMISSION_KEYS` are granted *globally*. For project-scoped permission
  keys, Jira evaluates this as "true if true in at least one project" — it can
  be `true` here while `false` for a specific project. `error` if none granted.
- **`projects`** — for every project visible to the account
  (`JiraClient::list_projects`, paginated `/project/search`), reports
  `service_user_permissions` (per-project `GET /mypermissions?projectKey=...`,
  can differ from `service_user`'s global list) and `service_user_roles` (which
  project roles the account belongs to, via `/project/<key>/role` +
  per-role actor lists, matched against the account's `accountId`).
  `status` is based only on `service_user_permissions` (`error` if empty);
  `service_user_roles` is `null` with `service_user_roles_note` explaining why
  if listing roles 401s — that endpoint requires "Administer Projects", which
  the account may not have everywhere. Zero visible projects is itself an
  `error` — an account that can't see any project can't do anything useful.

`PERMISSION_KEYS` includes `USER_PICKER` ("Browse users and groups"), needed by
`jira user search` and comment mentions. Unlike the other keys it's a
global-only permission (not part of any project's permission scheme), so it
reads identically in every project's `projects` entry — included anyway
because `user search` fails silently (empty match list, not an error) without
it, which is otherwise invisible to the caller. `ASSIGN_ISSUES` ("Assign
Issues") is needed by `jira issue assign`.

## Implemented commands

| Command | Notes |
|---------|-------|
| `init [--client-id --client-secret]` | Human onboarding; only command with narrative output |
| `doctor` | Cascading JSON health check (app_config, credentials, api, oauth_scopes, service_user, projects); exit non-zero on any failure |
| `auth login [--user]` | Default: `client_credentials` (service account, no browser). `--user`: interactive 3LO + PKCE |
| `auth whoami` | GET /myself |
| `issue get <KEY>` | Fetch single issue |
| `issue create` | POST with ADF description/body |
| `issue delete <KEY> --confirm` | Requires explicit confirmation flag |
| `issue transitions <KEY>` | List available workflow transitions |
| `issue transition <KEY> --to <STATUS>` | Case-insensitive match; lists valid options on mismatch |
| `issue assign <KEY> --assignee <ACCOUNT_ID> \| --unassign` | PUT with `{accountId}` (or `null` to unassign); exactly one of the two flags required |
| `issue search --jql [--stale-days N]` | Paginated JQL search. `--stale-days N` adds `AND updated <= -Nd` to `--jql` (inserted before `ORDER BY` if present) — JQL's own relative-date syntax, no separate staleness API needed |
| `issue comment add <KEY> --body [--mention ACCOUNT_ID]` | POST with ADF body; `--mention` and/or `{{mention:ACCOUNT_ID}}` inside `--body` add ADF mention nodes |
| `issue comment remove <KEY> <ID>` | DELETE |
| `user search --query <TEXT>` | GET /user/search; requires "Browse users and groups" (`USER_PICKER`), fails silently (empty list) without it |
| `project search --query <TEXT>` | GET /project/search; literal substring/prefix filter on key+name, no dedicated permission beyond what `doctor`'s `projects` check already exercises |

## Known edge cases (see BACKLOG.md)

FIELDS-1..4, AUTH-1..2, CREATE-1..2, DELETE-1.
