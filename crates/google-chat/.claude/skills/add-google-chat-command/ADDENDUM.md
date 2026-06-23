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

No e2e tests yet. Google Chat has no equivalent of a disposable test
project/site the way Jira does — testing against a real Workspace risks
polluting real spaces with test messages. Until an e2e strategy is agreed
with the user (e.g. a dedicated test space, cleaned up via a guard pattern
like jira's `IssueGuard`), rely on unit tests for parsing/CLI logic and
manual verification (`cargo run -p google-chat -- ...` against the real
Workspace) before merging.

## BACKLOG prefix

Use `GCHAT-` for entries added to `BACKLOG.md` for this crate.
