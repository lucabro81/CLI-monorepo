# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).
## [0.2.1](https://github.com/lucabro81/CLI-monorepo/compare/jira-v0.2.0...jira-v0.2.1) - 2026-07-15

### Fixed
- *(jira)* check --confirm before authenticating on issue delete

### Other
- replace release-plz with git-cliff + cargo-release
- *(jira)* load e2e test config from workspace .env

## [Unreleased]

## [0.1.2](https://github.com/lucabro81/CLI-monorepo/compare/jira-v0.0.1...jira-v0.1.2) - 2026-06-23

### Added

- *(jira)* doctor reports oauth scopes, global and per-project permissions
- add new-cli-crate skill and scaffold script
- *(jira)* add permissions check to doctor via mypermissions
- *(jira)* add OAuth2 client_credentials login for service accounts
- *(jira)* add jira init onboarding command
- pcke flow auth for atlassian app, get issue command

### Fixed

- *(jira)* bump version to 0.1.2 for the charset fix
- *(jira)* declare utf-8 charset on OAuth callback page
- *(jira)* e2e_cleanup JQL never matched orphaned issues
- *(jira)* renew service-account credentials in doctor instead of refresh
- *(jira)* correct pagination parameter name pageToken → nextPageToken

### Other

- release
- release v0.1.0
- add CI, release, and release-plz workflows ([#1](https://github.com/lucabro81/CLI-monorepo/pull/1))
- remove unused docs
- *(jira)* resolve DELETE-2, document OAuth scope vs permission scheme
- *(jira)* e2e_cleanup fails loudly when deletes don't succeed
- align root structure convention with actual crate layout
- document two-level test split in jira/bitbucket CLAUDE.md
- *(jira)* move test files into src/tests/
- clarify addendum step-numbering convention for agents
- align e2e-test addendum structure across jira and bitbucket
- trim addendum duplication with per-crate CLAUDE.md
- unify add-jira-command/add-bitbucket-command into shared skill
- *(jira)* tighten add-jira-command skill per review feedback
- *(jira)* remove hardcoded local home path from skill doc
- *(jira)* add add-jira-command skill for new command workflow
- *(jira)* centralize hardcoded API URLs and paths into endpoints.rs
- *(jira)* document e2e test prerequisites and running instructions
- *(jira)* add e2e test suite with IssueGuard and cleanup command
- *(jira)* after_help on all commands, pub doc comments, split CLAUDE.md
- *(jira)* reorganize into commands/ and add module-level docs
- *(jira)* extract issue commands into issue.rs
- Add jira doctor command
- Update README and CLAUDE.md for --select rename and issue search
- Add issue search; rename --fields to --select
- Test coverage, BACKLOG, and docs update
- Add issue create and issue delete commands
- Add edge case tests and BACKLOG.md
- Add --fields flag for selective JSON output
- Add issue transition and transitions commands
- Add issue comment add/remove commands
- Add auth whoami, refactor error handling and project structure
- new jira cli readme with onboarding of a atlassian and first commands, add design line guides to CLAUDE.md
- first commit

## [0.1.1](https://github.com/lucabro81/CLI-monorepo/compare/jira-v0.1.0...jira-v0.1.1) - 2026-06-22

### Other

- update Cargo.toml dependencies

## [0.1.0](https://github.com/lucabro81/CLI-monorepo/compare/jira-v0.0.1...jira-v0.1.0) - 2026-06-21

### Added

- *(jira)* doctor reports oauth scopes, global and per-project permissions
- add new-cli-crate skill and scaffold script
- *(jira)* add permissions check to doctor via mypermissions
- *(jira)* add OAuth2 client_credentials login for service accounts
- *(jira)* add jira init onboarding command
- pcke flow auth for atlassian app, get issue command

### Fixed

- *(jira)* e2e_cleanup JQL never matched orphaned issues
- *(jira)* renew service-account credentials in doctor instead of refresh
- *(jira)* correct pagination parameter name pageToken → nextPageToken

### Other

- add CI, release, and release-plz workflows ([#1](https://github.com/lucabro81/CLI-monorepo/pull/1))
- remove unused docs
- *(jira)* resolve DELETE-2, document OAuth scope vs permission scheme
- *(jira)* e2e_cleanup fails loudly when deletes don't succeed
- align root structure convention with actual crate layout
- document two-level test split in jira/bitbucket CLAUDE.md
- *(jira)* move test files into src/tests/
- clarify addendum step-numbering convention for agents
- align e2e-test addendum structure across jira and bitbucket
- trim addendum duplication with per-crate CLAUDE.md
- unify add-jira-command/add-bitbucket-command into shared skill
- *(jira)* tighten add-jira-command skill per review feedback
- *(jira)* remove hardcoded local home path from skill doc
- *(jira)* add add-jira-command skill for new command workflow
- *(jira)* centralize hardcoded API URLs and paths into endpoints.rs
- *(jira)* document e2e test prerequisites and running instructions
- *(jira)* add e2e test suite with IssueGuard and cleanup command
- *(jira)* after_help on all commands, pub doc comments, split CLAUDE.md
- *(jira)* reorganize into commands/ and add module-level docs
- *(jira)* extract issue commands into issue.rs
- Add jira doctor command
- Update README and CLAUDE.md for --select rename and issue search
- Add issue search; rename --fields to --select
- Test coverage, BACKLOG, and docs update
- Add issue create and issue delete commands
- Add edge case tests and BACKLOG.md
- Add --fields flag for selective JSON output
- Add issue transition and transitions commands
- Add issue comment add/remove commands
- Add auth whoami, refactor error handling and project structure
- new jira cli readme with onboarding of a atlassian and first commands, add design line guides to CLAUDE.md
- first commit
