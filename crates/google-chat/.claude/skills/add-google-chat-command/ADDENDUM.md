# `google-chat` crate specifics for add-cli-command

Read alongside `.claude/skills/add-cli-command/SKILL.md` (workspace root) and
`crates/google-chat/CLAUDE.md` (already covers module map, OAuth/auth
design, API design notes, command table — don't repeat that here, only what's
missing for the skill's process).

Section headings below match `SKILL.md`'s step numbers — only steps where
this crate deviates from or adds to the generic skill are covered here.
Steps not listed (3-5, 7-9) follow `SKILL.md` as-is.

## Step 1 — Scope

- **Scope check**: there is no per-call permission map like jira's
  `PERMISSION_KEYS` — Google Chat authorizes by OAuth scope only. If the new
  command needs a scope not already requested
  (`chat.spaces.readonly`/`chat.messages.readonly`/`chat.messages.create`),
  add it to `OAuthConfig::SCOPES` in `auth.rs` and to the Setup section of
  `README.md`.
- A new scope requires re-running `auth login` (or `init`) — a one-time
  human consent step, same as jira's `--user` flow re-consent.

## Step 2 — API research

Docs: `https://developers.google.com/workspace/chat/api/reference/rest`
(use WebFetch/WebSearch).

## Step 6 — e2e tests

`crates/google-chat/src/tests/e2e_tests.rs`, each test `#[ignore = "e2e: requires credentials"]`:

- **Read-only only**: Google Chat has no equivalent of a disposable test
  project/site the way Jira does, and `spaces list`/`messages list` are the
  only commands e2e-covered so far — they touch nothing. `messages send`
  creates real, visible messages in spaces shared with real colleagues, and
  is deliberately **not** e2e-covered automatically — see `BACKLOG.md`
  GCHAT-2. Don't add an automated/repeated e2e test that sends messages to a
  real space without explicit user sign-off (and only after the people in
  that space have been told what's being tested).
- **No isolation/cleanup needed**: unlike jira's `IssueGuard`, these tests
  create nothing, so there's no teardown pattern to follow.
- **Running**:
  ```sh
  cargo test -p google-chat -- --ignored
  ```
- **Extending**: for a new read-only command, add a test following
  `e2e_messages_list_on_first_space_succeeds`'s pattern — call `spaces.list`
  to discover a real target rather than hardcoding a space id, and assert
  the response is well-formed (right top-level keys/types) rather than
  asserting specific content, since real account data will vary over time.

## BACKLOG prefix

Use `GCHAT-` for entries added to `BACKLOG.md` for this crate.
