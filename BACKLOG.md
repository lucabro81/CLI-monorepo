# Backlog

Known edge cases, deferred fixes, and design notes. Each entry records what was found,
the current behaviour, why it was deferred, and what a future fix would look like.

---

## `crates/jira`

### fields.rs (now `crates/cli-fields` — extracted as a shared library, see root CLAUDE.md)

The entries below were found while `fields.rs` still lived per-crate (identical
in jira/bitbucket/google-chat); the code was since extracted to the shared
`crates/cli-fields` crate, so these apply to all three crates via `filter_fields`
there, not just jira.

#### FIELDS-1 — Empty string field path silently returns empty object
**Found:** review session 2026-06-09  
**Trigger:** `--fields ","` or `--fields "summary,"` → `split(',')` produces `""` entries  
**Current behaviour:** empty string becomes a key lookup for `""` in the JSON object; never matches; silently omitted → caller gets `{}` with no indication something went wrong  
**Acceptable?** Barely. An LLM won't pass `--fields ","` intentionally, but a trailing comma is plausible.  
**Future fix:** strip empty segments after split in `main.rs` (already `trim`-ed, add `filter(|s| !s.is_empty())`); or validate in `filter_fields` and surface an error.

---

#### FIELDS-2 — All requested fields missing → silent empty object `{}`
**Found:** review session 2026-06-09  
**Trigger:** `--fields nonexistent` on any response  
**Current behaviour:** returns `{}` — structurally valid JSON, but the caller has no idea whether the field doesn't exist or the response was empty.  
**Acceptable?** Yes for now. An LLM can detect `{}` and re-query without `--fields`. A future improvement could warn to stderr when the filtered result is empty.  
**Future fix:** if `fields` non-empty and filtered result is `{}`, print a warning to stderr listing the requested paths.

---

#### FIELDS-3 — Intermediate null on nested path returns null, not omitted
**Found:** review session 2026-06-09  
**Trigger:** `--fields status.name` on `{"status": null}`  
**Current behaviour:** `apply_tree(null, {name:{}})` hits the `other => other.clone()` arm → returns `null` → result is `{"status": null}`  
**Acceptable?** Yes. Null is valid JSON; the field exists but has no value. Consistent with how top-level nulls are handled.  
**Future fix:** none planned.

---

#### FIELDS-4 — Intermediate scalar on nested path returns scalar unchanged
**Found:** review session 2026-06-09  
**Trigger:** `--fields status.name` on `{"status": "open"}` (status is a string, not an object)  
**Current behaviour:** `apply_tree("open", {name:{}})` → returns `"open"` → result is `{"status": "open"}`. The `.name` segment is silently ignored.  
**Acceptable?** Marginal. The caller asked for `status.name` but gets the whole `status` value. Could be surprising if the API changes a field from scalar to object.  
**Future fix:** when the value at a non-leaf node is neither object nor array, either omit the key from the result or emit a stderr warning.

---

### auth.rs

#### AUTH-1 — Callback error param not surfaced clearly
**Found:** review session 2026-06-09  
**Trigger:** Atlassian redirects with `?error=access_denied&error_description=...` instead of `?code=...`  
**Current behaviour:** `parse_callback_request_line` returns `MissingParam("code")` — technically correct but the actual error reason (e.g. user denied consent) is in the `error` param which we never read.  
**Acceptable?** For now. The user sees "OAuth login failed: invalid OAuth callback: MissingParam("code")" — not great but rare path.  
**Future fix:** in `wait_for_callback`, after parsing params check for an `error` key and surface `error_description` as a dedicated `LoginError::ConsentDenied(String)` variant.

---

### issue create / issue delete

#### CREATE-1 — Empty `--summary` accepted by CLI, rejected by Jira with opaque 400
**Found:** review session 2026-06-09  
**Trigger:** `jira issue create --project KAN --type Task --summary ""`  
**Current behaviour:** clap parses it, Jira returns 400 with a field-validation error that surfaces as `ApiError { status: 400, body: ... }`. The body is Jira's raw JSON error, not particularly LLM-friendly.  
**Acceptable?** Marginal. Rare in practice; Jira's error body does explain the problem.  
**Future fix:** validate non-empty in `run_issue` before the API call; return `CliError::InvalidInput` with "summary must not be empty".

---

#### CREATE-2 — Wrong `--type` gives Jira 400, no list of available types shown
**Found:** review session 2026-06-09  
**Trigger:** `jira issue create --project KAN --type "NonExistent" --summary "x"`  
**Current behaviour:** Jira returns 400; raw error body shown. No list of valid types.  
**Acceptable?** Yes for now. Unlike transitions (where valid options depend on issue state), issue types per project are stable and discoverable via `GET /rest/api/3/project/{key}/issuetypes`. Could add an `issue types <PROJECT>` command later.  
**Future fix:** add `issue types <PROJECT>` command to list available types; reference it in the `issue create` help text.

---

#### DELETE-1 — Missing `--delete-subtasks` on issue with subtasks gives Jira 400
**Found:** review session 2026-06-09  
**Trigger:** `jira issue delete KAN-X --confirm` where KAN-X has subtasks  
**Current behaviour:** Jira returns 400; raw error body shown. The `--delete-subtasks` flag is documented in `--help` but the error doesn't remind the caller about it.  
**Acceptable?** Yes. The flag is explicit in `--help` and the `after_help` example. A 400 body from Jira typically mentions subtasks.  
**Future fix:** detect "subtask" in the 400 response body and surface a tailored `CliError` that mentions `--delete-subtasks`.

---

#### DELETE-2 — RESOLVED: service account had no project role in `JIRA_E2E_PROJECT`, breaking e2e cleanup
**Found:** 2026-06-12, while investigating leftover `[jira-cli-e2e]` issues in KAN  
**Trigger:** the `client_credentials` service account (`mercury`) used by e2e tests was not a member of any project role in KAN. `DELETE_ISSUES` in the permission scheme is granted to a project role (e.g. "Administrators"), not to individual accounts, so `mycount` had no path to that permission regardless of OAuth scopes.  
**Symptom:** `IssueGuard::drop`'s `delete_issue` call returned 403 (silently swallowed); `e2e_cleanup` got 403 for every issue, reporting "deleted 0 issue(s)" — e2e-created issues accumulated indefinitely (KAN-44..93 observed, 50 issues).  
**Resolved:** 2026-06-13 — added `mercury` to the project role with `DELETE_ISSUES` in KAN (Project settings → People). Jira's built-in **permission helper** ("why can't I...") was the fastest way to diagnose this — given user + issue + permission it states exactly which permission-scheme condition fails. `e2e_cleanup` then deleted all 50 orphans successfully; `e2e_cleanup` now also fails loudly (asserts) if any delete fails, so this won't silently regress again.  
**See:** [crates/jira/docs/oauth-scopes-vs-permissions.md](crates/jira/docs/oauth-scopes-vs-permissions.md) for the scope-vs-permission-scheme distinction.

---

#### AUTH-2 — `OAuthConfig` does not validate non-empty client_id / client_secret
**Found:** review session 2026-06-09  
**Trigger:** `app.json` with `{"client_id": "", "client_secret": ""}` — parses successfully  
**Current behaviour:** empty strings pass `from_json`; the error surfaces later as a 401 from Atlassian with a generic message.  
**Acceptable?** Marginal. Early validation would give a clearer error.  
**Future fix:** add validation in `OAuthConfig::from_json` — return `InvalidJson` (or a new `EmptyCredential` variant) if either field is blank.

---

### DOCTOR-1 — `permissions` check's fixed boolean map is arbitrary; consider reporting raw permissions instead
**Found:** 2026-06-11, while building bitbucket's `doctor` permissions check  
**Context:** jira's `doctor` `permissions` check (`PERMISSION_KEYS` + `mypermissions`) reports a fixed map of booleans for permissions the CLI happens to rely on today, with `status: ok` gated arbitrarily on `BROWSE_PROJECTS`. When designing the bitbucket equivalent, we initially copied this pattern (fixed scope list + booleans) but decided it added little value: the "required" list is arbitrary, drifts from reality as commands are added, and hides the actual granted permissions. bitbucket's `permissions` check now just reports `granted_scopes` as-is (`status: error` only if empty).  
**Possible direction:** simplify jira's `permissions` check the same way — report the raw `mypermissions` response (or the granted permission keys) instead of a fixed boolean map, with `status: error` only if essentially nothing is granted (e.g. `BROWSE_PROJECTS` false, the one permission that gates everything else).  
**Add when:** next time `crates/jira/src/commands/doctor.rs` is touched — not worth a standalone change right now.




### COMMENT-1 — Add `issue comment get <KEY> <COMMENT_ID>` command
**Context:** currently the only way to retrieve a specific comment is via `issue get <KEY> --select fields.comment.comments`, which downloads the full issue. Jira exposes `GET /rest/api/3/issue/{key}/comment/{id}` returning the same comment object in isolation.  
**When useful:** issues with many comments where fetching the full issue is wasteful; LLM workflows that store a comment ID and need to re-read it later.  
**Current workaround:** `issue get <KEY> --select fields.comment.comments` — sufficient for the common case.  
**Add when:** a concrete performance or usability issue is observed in practice.

---

### SEARCH-1 — Add convenience flags as JQL shorthands
**Context:** `issue search` currently requires full JQL syntax. Common patterns like filtering by assignee, project, or status could be expressed as dedicated flags (`--assignee`, `--project`, `--status`) compiled into JQL internally.  
**When useful:** if the target LLM struggles with JQL syntax or if certain patterns appear so frequently that a shorthand reduces friction meaningfully.  
**Current approach:** JQL only — LLMs trained on Jira data handle it well and Jira returns clear syntax errors for self-correction.  
**Add when:** a recurring pattern proves awkward in practice (e.g. "find my open issues" typed repeatedly).

---

### HELP-1 — Trim verbose flag descriptions in the Options section
**Context:** CLI is intended to be driven by a local LLM with limited context. The Options section is generated automatically by clap and cannot be removed, but individual flag *descriptions* can be stripped where the flag name is self-explanatory.  
**Approach:** keep descriptions only where there is a non-obvious constraint (default value, cap, special format, side effect). Move everything else to `after_help` examples. Full human-readable documentation stays in the README.  
**Priority:** low — context windows are reasonable even on local models. Revisit if targeting models with narrow windows (< 8k).

---

### SKILL-1 — Generalize/restrict `add-cli-command` skill for non-Claude-Code agents
**Context:** `.claude/skills/add-cli-command/SKILL.md` (workspace root, originally added as `crates/jira/.claude/skills/add-jira-command/SKILL.md` on 2026-06-10, generalized to a shared root skill with per-crate `ADDENDUM.md` files on 2026-06-11) references Claude-Code-specific tools (`AskUserQuestion`, `WebFetch`/`WebSearch`) and assumes the executing agent can read arbitrary repo files (`CLAUDE.md`, `BACKLOG.md`, `ADDENDUM.md`) and run a multi-step unsupervised loop reliably.  
**Risk:** a mid-size local model (30-70B) via Ollama or another provider may not recognize these tools/files at all, silently skip steps that depend on them (e.g. the initial scoping questions), or fail to sustain the long verification loop.  
**Possible directions:** (a) generalize tool references to "ask the user, using whatever clarification mechanism is available" / "use available web research tools"; (b) add a leaner variant of the skill scoped to what a 30-70B model can reliably execute (fewer steps, more explicit checkpoints, less reliance on long unsupervised loops).  
**Add when:** there's an actual attempt to run this skill with a non-Claude-Code agent or a smaller model — don't generalize speculatively before that.

---

### client.rs

#### CLIENT-1 — No handling for Jira API rate limiting (HTTP 429)
**Found:** review session 2026-06-10  
**Trigger:** an agent issuing many requests in quick succession (e.g. bulk operations, tight retry loops) hits Jira Cloud's rate limit.  
**Current behaviour:** `ClientError::Status { status: 429, body }` surfaces as a generic "Jira returned status 429: ..." — no indication of `Retry-After`, no distinction from other 4xx errors.  
**Acceptable?** Yes for now — current command set is low-volume, single-request-per-invocation.  
**Future fix:** read the `Retry-After` header and surface it in the error message ("rate limited, retry after Ns") so an agent can self-correct by waiting; consider a dedicated `ClientError::RateLimited { retry_after_secs }` variant.

---

#### CLIENT-2 — `cloud_id` resolution picks the first accessible resource arbitrarily
**Found:** review session 2026-06-10  
**Trigger:** an Atlassian account/app with access to more than one Jira Cloud site — `fetch_cloud_id` (auth.rs) takes `resources.into_iter().next()`.  
**Current behaviour:** silently picks whichever site the accessible-resources endpoint lists first; no way to target a different site.  
**Acceptable?** Yes — current setup (and documented setup flow) assumes a single Jira site per app/account.  
**Future fix:** if multi-site support is ever needed, add a `--site` flag or `JIRA_SITE` config value, and have `fetch_cloud_id` match against it (erroring with the list of available sites if not found/ambiguous).

---

## Cross-crate

### AUTH-3 (bitbucket) — 3LO/PKCE "human" auth flow not needed, deferred
**Found:** 2026-06-11, design discussion  
**Context:** considered mirroring jira's `auth login --user` (3LO + PKCE) for bitbucket.  
**Why deferred:** in jira, `--user` exists mainly as a one-time bootstrap — a human must grant the OAuth app consent/installation on the site before `client_credentials` has any scope (see `jira init`). Bitbucket's workspace-level OAuth consumer is granted permissions directly at creation time; `client_credentials` works standalone with no bootstrap step. So bitbucket has less need for 3LO than jira does, not more.  
**Add when:** a concrete use case appears that `client_credentials`/workspace identity can't satisfy (e.g. accessing personal repos outside the workspace, or an action Bitbucket restricts to user identities).

---

### LIB-1 — Shared library for Atlassian-product OAuth/config code
**Found:** 2026-06-11, after implementing `crates/bitbucket/src/auth.rs`  
**Context:** `crates/bitbucket/src/auth.rs` duplicates patterns from `crates/jira/src/auth.rs` — config dir resolution (`$XDG_CONFIG_HOME/<cli>-cli/`), `app.json`/`credentials.json` layout, `OAuthConfig`/`Credentials`/`LoginError`/`now_unix()` naming and structure — simplified for `client_credentials` (no PKCE/refresh_token/cloud_id). Deliberately duplicated for now ("duplica codice per ora, quando è fatta vediamo se riusciamo ad astrarre").  
**Risk if deferred too long:** a third Atlassian (or similar OAuth-shaped) CLI would triple the duplication and make fixing a bug (e.g. token-expiry leeway, config-path bug) require touching N crates.  
**Possible direction:** extract a small workspace-local crate (e.g. `crates/atlassian-auth` or more general `crates/oauth-cli-support`) covering: config dir resolution, `app.json`/`credentials.json` read/write, `now_unix()`, and a couple of grant-flow helpers (client_credentials now, 3LO+PKCE for jira). Keep it generic enough that a non-Atlassian product (e.g. a future GitHub/GitLab CLI) could reuse the config-path + credentials-file parts even if the OAuth specifics differ.  
**Priority:** medium — not urgent, but don't let a third crate get built with a third copy-paste before this is addressed.

---

### PR-1 (bitbucket) — `pr list` default-state behavior unverified live
**Found:** 2026-06-11, while implementing `pr list`  
**Context:** docs say omitting `--state` returns pull requests in any state. Both test repos (`lucabrognaracode/repo-test`, `lucabrognaracode/cli-test-repo`) currently have zero pull requests in any state, so `pr list` (with and without `--state`) only returned `{"page":1,"pagelen":10,"size":0,"values":[]}` — the empty-result shape, endpoint path, and query-param wiring (`--state`, `--page`) were verified, but the actual filtering behavior of `--state` and the default-no-filter behavior were not observed against real data.  
**Add when:** a repo with pull requests in mixed states becomes available — re-run `pr list` with and without `--state` and confirm the docs/help text match observed behavior.

---

### PR-2 (bitbucket) — `pr create --reviewers` flag deferred, needs UUID lookup
**Found:** 2026-06-12, design discussion for `pr create`
**Context:** `pr create` was implemented without a `--reviewers` flag. Bitbucket's `reviewers` field on `POST .../pullrequests` expects a list of account objects identified by `uuid` (or `account_id`/`username`, deprecated) — not human-friendly display names, so an LLM caller would need a way to resolve a person to a UUID first (e.g. a `workspace members` lookup command that doesn't exist yet).
**Why deferred:** v1 covers the no-reviewer case; reviewers add a dependency on a lookup command that's out of scope for the current `pr` command batch.
**Add when:** reviewer assignment is actually needed in a workflow — likely pairs with adding a `workspace members list` (or similar) command so an LLM can resolve a username to a `uuid` first, then pass `--reviewers <uuid1,uuid2,...>` through as-is in the request body.

---

### REPO-1 (bitbucket) — `repo update`/`repo edit` command, raw JSON body vs flags
**Found:** 2026-06-11, design discussion for `repo create`  
**Context:** `repo create` was implemented with typed flags (`--description`, `--private`, `--project`), matching jira's `issue create` convention — only ~9 settable fields on `POST /2.0/repositories/{workspace}/{repo_slug}`, most rarely used. A future `repo update` (`PUT` on the same endpoint, supports a larger/overlapping set of fields plus things like `fork_policy`, `language`, `has_issues`, `has_wiki`, `mainbranch`) might instead take a single `--body <JSON>` (or stdin) parameter passed through as-is, since enumerating a flag per field gets unwieldy for an edit command that may touch any subset of fields.  
**Add when:** `repo update`/`repo edit` is actually implemented — decide then whether typed flags (consistent but verbose) or a raw JSON body (flexible, less discoverable via `--help`) fits better; could also revisit `repo create` for consistency at that point.

---

### TESTENV-1 — RESOLVED: `.env`-based e2e test-target configuration, across crates
**Found:** 2026-07-14, while adding `google-chat messages delete`
**Context:** every crate's e2e tests need a real, pre-agreed test target, and today that's handled two different, both-imperfect ways: jira reads `JIRA_E2E_PROJECT` from an inline env var supplied on the command line each run (`JIRA_E2E_PROJECT=KAN cargo test -p jira -- --ignored`), not persisted anywhere in the repo; google-chat has no env var at all — its equivalent test space (`spaces/AAQAtCLmaho`, `spaces/ud85UsAAAAE`) lives only in conversational memory across sessions, not in the codebase. No crate uses a `.env` file or the `dotenvy` crate (confirmed: no such dependency anywhere in `Cargo.lock`, no `.env*` file in the repo).
**User's ask:** a proper `.env`-based convention, applying to jira and google-chat at minimum, so e2e test targets are configured once and persisted in the environment/repo (e.g. a checked-in `.env.example` + a gitignored local `.env`) rather than re-typed per invocation or kept as tribal knowledge.
**Why it matters beyond convenience:** for `google-chat`, this is also a prerequisite for closing GCHAT-2 — a safe, explicit, self-cleaning e2e test for `messages send`+`messages delete` needs a specific known space id available to the test code, not just "first space returned by `spaces.list`".
**Resolved 2026-07-15:** added `dotenvy` as a `[dev-dependencies]` entry to jira and google-chat; a workspace-root `.env.example` (checked in, placeholder values) documents `JIRA_E2E_PROJECT` and `GOOGLE_CHAT_E2E_SPACE`; `.env` itself is gitignored and holds the real local values (`KAN`, `spaces/AAQAtCLmaho`). Both crates' `src/tests/e2e_tests.rs::setup()` call `dotenvy::dotenv().ok()` first, so `.env` is loaded automatically — an already-exported env var still takes precedence (dotenvy never overrides an existing value), so the old inline-per-run style keeps working unchanged. `google-chat` also gained a `test_space()` helper (mirroring jira's `project_key()`) and a new read-only e2e test, `e2e_messages_list_on_designated_test_space_succeeds`, giving `GOOGLE_CHAT_E2E_SPACE` a real consumer instead of sitting unused. **Verified live**: ran `e2e_messages_list_on_designated_test_space_succeeds` with both `JIRA_E2E_PROJECT`/`GOOGLE_CHAT_E2E_SPACE` explicitly unset in the shell — it passed, confirming the value came from `.env`, not a leftover export.
**Residual, not done:** `new-cli-crate`'s scaffold was not updated to wire this in by default for future crates — still copy-paste-and-adapt for now. Add when a third crate needs e2e tests.
**See also:** GCHAT-2 (still open — `GOOGLE_CHAT_E2E_SPACE` existing doesn't by itself authorize automated `messages send`/`delete`; that still needs explicit confirmation the designated space is safe for repeated automated message cycles, not just read-only checks).

---

## `crates/google-chat`

### GCHAT-1 — Service-account/domain-wide-delegation login not yet activated
**Found:** 2026-06-23, during `auth login` implementation
**Context:** `auth login` (default, no flags) implements the full service-account + domain-wide-delegation (DWD) flow — JWT-bearer assertion impersonating a Workspace "service user" — and is unit-tested, but it cannot be exercised live yet. It requires a Workspace super-admin to (1) enable "Google Workspace Domain-wide Delegation" on the service account and (2) authorize its Client ID + scopes in Admin Console. The current operator doesn't have super-admin access and can't request it right now (on leave; would also have to explain/justify the agent's access, which could lead to the request being delayed or redirected through the company).
**Current behaviour:** `auth login --user` (interactive OAuth 2.0 + PKCE, logging in as a human Google account) is the working day-to-day path and is what the crate actually runs on for now. The default (no-flags) service-account path is dormant — present and tested, but unused.
**This is not abandoned** — it's the intended path once admin access is available, just not now. Activating it later needs no code changes: just complete the two admin steps above and add the `service_account` block to `app.json` (see `crates/google-chat/README.md` Setup step 5). `write_app_config` already preserves a hand-added `service_account` block across `init` reruns for exactly this reason.
**Add when:** super-admin access becomes available — complete the DWD admin setup, verify `auth login` (no flags) live, and update `crates/google-chat/CLAUDE.md`/`README.md` "Implemented commands" to note it's been verified end-to-end.

---

### GCHAT-2 — `messages send`/`messages delete` deliberately have no automated e2e test
**Found:** 2026-06-23, while adding read-only e2e tests for `spaces list`/`messages list`; extended 2026-07-14 when `messages delete` was added
**Context:** `crates/google-chat/src/tests/e2e_tests.rs` covers `spaces.list` and `messages.list` (read-only, no side effects). `messages send` is excluded on purpose: it creates a real, visible message in a real space shared with a real person — currently the manual live test target is `spaces/ud85UsAAAAE`, a DM with a colleague who's aware test messages might appear there occasionally, but who has **not** been told this could become an automated/repeated test. `messages delete` is excluded for the same reason, and more conservatively still — it permanently removes a real message with no undo, so even a self-cleaning e2e test needs a space where automated create+delete cycles are known and accepted, not just tolerated.
**Current behaviour:** `messages send` and `messages delete` are verified only via manual `cargo run` smoke tests during development, not by any test that runs as part of `cargo test`.
**Add when:** the user confirms a specific space is designated and safe for repeated automated `messages send`+`messages delete` cycles — `GOOGLE_CHAT_E2E_SPACE`/`.env` (see the cross-crate `TESTENV-1` entry below, now resolved) already has a place to record that space id once this is unblocked — then add an `#[ignore]` e2e test following jira's `IssueGuard`-style self-cleaning pattern: send a disposable message, assert on it, delete it via `messages delete` in teardown.

---

### GCHAT-3 — RESOLVED: `subscription create`/`listen` verified live end-to-end
**Found:** 2026-06-24, while implementing `subscription create` and `listen`
**Context:** added two new OAuth scopes (`chat.memberships.readonly`, `pubsub`) and two new commands: `subscription create` (ensures a Pub/Sub pull subscription exists, then creates a Workspace Events API subscription delivering Chat message events for a space to a Pub/Sub topic) and `listen` (streams messages from that Pub/Sub subscription via `google-cloud-pubsub`, the crate's only async command).
**Verified live 2026-06-25** against the real `mercury-500017` project: `auth login --user` re-consented to the new scopes without issue; `subscription create --space spaces/ud85UsAAAAE --topic projects/mercury-500017/topics/mercury-chat-events --pubsub-subscription projects/mercury-500017/subscriptions/mercury-chat-events-sub` returned `state: ACTIVE` (so `chat.spaces.readonly` + `chat.memberships.readonly` are indeed sufficient — no extra scope needed); `listen` on that subscription printed the full Workspace Events CloudEvent envelope (with `data.message`, the real Chat message, since `includeResource: true`) within ~2s of a real `messages send`; `kill -TERM <pid>` (the printed PID) exited the process cleanly (exit code 0) — confirms the SIGTERM shutdown path works for an agent/script stopping it as a background job, not just Ctrl+C.
**Not separately exercised:** the background token-refresh loop (5-minute poll of `context::authenticated_credentials()`) — the live test ran for under a minute, well short of the ~1h token lifetime or even one 5-minute poll interval. The code path itself (`spawn_blocking(authenticated_credentials)`) reuses the same renewal logic already verified live for every other command, so risk is low, but it has not been observed firing during an actual `listen` run.
**See also:** GCHAT-4 (Workspace Events subscriptions expire after ~4h, no renewal command exists yet).

---

### GCHAT-4 — RESOLVED: Workspace Events subscriptions expire after ~4h; `listen` now auto-renews
**Found:** 2026-06-25, live verification of `subscription create`
**Context:** the live `subscription create` response included `"expireTime": "2026-06-25T18:21:04Z"` for a subscription created at `"createTime": "2026-06-25T14:21:07Z"` — roughly 4 hours later. This is the Workspace Events API's own default TTL for a subscription, not something this CLI sets. Once a subscription expires, Google stops delivering events for it; `listen` itself would keep running (it doesn't know the subscription expired — it's just an idle Pub/Sub pull), but no new messages would ever arrive again until a fresh `subscription create` was run.
**Resolved 2026-06-25:** `listen` now takes a required `--workspace-events-subscription <name>` flag (the `name` field from `subscription create`'s output) and renews it automatically every 30 minutes via `EventsClient::renew_subscription` — `PATCH workspaceevents.googleapis.com/v1/{name}?updateMask=ttl` with body `{"ttl":"0s"}`, which resets the subscription to its maximum TTL. The 30-minute interval is a fixed safety margin, comfortably shorter than the observed ~4h TTL, rather than computed from the exact `expireTime` (avoids pulling in a date/time-parsing dependency for no real benefit). Runs alongside the existing token-refresh task in the same `tokio::select!` loop in `commands/listen.rs`.
**Verified live:** the PATCH call (tested directly, same code path as `EventsClient::renew_subscription`) returned `state: ACTIVE` with `expireTime` pushed out another ~4h from the renewal time, using the same OAuth scopes already granted (`chat.spaces.readonly` + `chat.memberships.readonly` + `pubsub`) — no extra scope needed. The automatic 30-minute trigger itself was not observed firing during a real `listen` run (would require leaving one running for 30+ minutes); the underlying PATCH call it invokes was verified directly instead.
**Residual, accepted as-is:** `subscription create` still creates a brand-new subscription resource each time it's run (it doesn't check for an existing one on the same space/topic), so resources from old `listen` sessions that were never renewed (e.g. an agent that exited without `listen` running long enough to renew, or was killed before the first 30-minute mark) will still expire on their own and accumulate as harmless leftover subscriptions. Not worth addressing without evidence it's an actual problem in practice.
**Update 2026-06-25:** added `subscription delete --name <name>` so an agent leaving a conversation can tighten access immediately instead of waiting for the natural ~4h expiry — see the access-scoping discussion in `crates/google-chat/CLAUDE.md`'s "API design notes" (per-space `subscription create` calls, not the `spaces/-` wildcard, paired with `subscription delete` on exit). Verified live: delete succeeded (`done: true`), and a repeat delete on the same name correctly came back `403 SUBSCRIPTION_ACCESS_DENIED`.