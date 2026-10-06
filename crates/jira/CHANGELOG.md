# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).
## [2.0.0](https://github.com/lucabro81/CLI-monorepo/compare/jira-v1.0.0...jira-v2.0.0) - 2026-10-06

### Added
- *(jira)* --user <id> selects a person, one credentials folder each, auth logout

### Fixed
- *(jira)* name the identity in the nothing-to-log-out error
- *(jira)* report an unwritable credentials file instead of asking to log in again

### Other
- *(jira)* list auth logout among the --select-exempt commands
- *(jira)* name the person's id in the remote login comments
- *(jira)* document --user <id>, per-person folders and auth logout (#175)
- *(jira)* explain the .lock file next to the credentials (#175)
- Merge pull request #176 from lucabro81/issue175 ([#176](https://github.com/lucabro81/CLI-monorepo/pull/176))
- Release
- Merge pull request #177 from lucabro81/release/jira ([#177](https://github.com/lucabro81/CLI-monorepo/pull/177))
## [2.0.0](https://github.com/lucabro81/CLI-monorepo/compare/jira-v1.0.0...jira-v2.0.0) - 2026-10-06

### Added
- *(jira)* --user <id> selects a person, one credentials folder each, auth logout

### Fixed
- *(jira)* name the identity in the nothing-to-log-out error
- *(jira)* report an unwritable credentials file instead of asking to log in again

### Other
- *(jira)* list auth logout among the --select-exempt commands
- *(jira)* name the person's id in the remote login comments
- *(jira)* document --user <id>, per-person folders and auth logout (#175)
- *(jira)* explain the .lock file next to the credentials (#175)
- Merge pull request #176 from lucabro81/issue175 ([#176](https://github.com/lucabro81/CLI-monorepo/pull/176))
## [1.0.0](https://github.com/lucabro81/CLI-monorepo/compare/jira-v0.9.0...jira-v1.0.0) - 2026-10-06

### Added
- *(atlassian-auth)* add Identity and sectioned AppConfig
- *(jira)* keep service and human credentials side by side, pick one per call with --user

### Fixed
- *(atlassian-auth)* refuse credentials that don't match the identity they are loaded as
- *(jira)* say so when init replaces an old single-identity app.json

### Other
- *(atlassian-auth)* drop the flat app.json API now that every caller uses AppConfig
- document side-by-side service and user identities
- fix stale references after the identity split
- *(jira)* remote login step 2 prints auth whoami --user
- Merge pull request #166 from lucabro81/issue164 ([#166](https://github.com/lucabro81/CLI-monorepo/pull/166))
## [0.9.0](https://github.com/lucabro81/CLI-monorepo/compare/jira-v0.8.2...jira-v0.9.0) - 2026-10-05

### Added
- *(jira)* add auth login --user --remote, a two-step login for someone elsewhere

### Fixed
- *(jira)* end every remote login error with how to restart it

### Other
- *(jira)* describe the shared callback listener
- Merge pull request #147 from lucabro81/issue143 ([#147](https://github.com/lucabro81/CLI-monorepo/pull/147))
- *(jira)* document the two-step remote login
- *(oauth-user-login)* make the pending login's redirect URI optional
- *(jira)* say in doctor --help that pending_login is informational
- Merge pull request #154 from lucabro81/issue146 ([#154](https://github.com/lucabro81/CLI-monorepo/pull/154))
## [0.8.2](https://github.com/lucabro81/CLI-monorepo/compare/jira-v0.8.1...jira-v0.8.2) - 2026-10-03

### Fixed
- *(jira)* drop currentUser() from issue search --help example

### Other
- *(jira)* steer assignee filtering away from currentUser() in README
- Merge pull request #139 from lucabro81/issue138 ([#139](https://github.com/lucabro81/CLI-monorepo/pull/139))
## [0.8.1](https://github.com/lucabro81/CLI-monorepo/compare/jira-v0.8.0...jira-v0.8.1) - 2026-10-02

### Fixed
- *(jira)* pass --select in every non-exempt command's --help example

### Other
- *(jira)* describe the mandatory --select contract in the README
- Merge pull request #134 from lucabro81/issue133 ([#134](https://github.com/lucabro81/CLI-monorepo/pull/134))
## [0.8.0](https://github.com/lucabro81/CLI-monorepo/compare/jira-v0.7.0...jira-v0.8.0) - 2026-08-07

### Added
- *(jira)* add browse_url to issue create/get via stored site URL

### Other
- *(jira)* document site_url/browse_url, fix stale fetch_cloud_id reference
- Merge pull request #112 from lucabro81/issue111 ([#112](https://github.com/lucabro81/CLI-monorepo/pull/112))
## [0.7.0](https://github.com/lucabro81/CLI-monorepo/compare/jira-v0.6.0...jira-v0.7.0) - 2026-08-06

### Added
- *(jira)* add --parent flag to issue create for subtasks/epics

### Other
- *(jira)* add e2e test for issue create --parent
- *(jira)* document --parent in CLAUDE.md
- Merge pull request #108 from lucabro81/issue106 ([#108](https://github.com/lucabro81/CLI-monorepo/pull/108))
## [0.6.0](https://github.com/lucabro81/CLI-monorepo/compare/jira-v0.5.1...jira-v0.6.0) - 2026-08-05

### Added
- *(jira)* convert Markdown to ADF for issue create/comment add (#93)

### Other
- remove resolved entries from BACKLOG.md
- Merge pull request #65 from lucabro81/chore/backlog-cleanup ([#65](https://github.com/lucabro81/CLI-monorepo/pull/65))
- migrate BACKLOG.md to GitHub issues
## [0.5.1](https://github.com/lucabro81/CLI-monorepo/compare/jira-v0.5.0...jira-v0.5.1) - 2026-08-03

### Fixed
- *(jira)* correctly report multi-product OAuth scopes in doctor

### Other
- *(jira)* extract shared Atlassian OAuth into crates/atlassian-auth
- Merge pull request #56 from lucabro81/feat/confluence-crate ([#56](https://github.com/lucabro81/CLI-monorepo/pull/56))
## [0.5.0](https://github.com/lucabro81/CLI-monorepo/compare/jira-v0.4.0...jira-v0.5.0) - 2026-07-30

### Added
- *(jira)* add project search command (#53) ([#53](https://github.com/lucabro81/CLI-monorepo/pull/53))
## [0.4.0](https://github.com/lucabro81/CLI-monorepo/compare/jira-v0.3.3...jira-v0.4.0) - 2026-07-30

### Added
- *(jira)* add user search and comment mentions (#47) ([#47](https://github.com/lucabro81/CLI-monorepo/pull/47))
- *(jira)* add issue assign/unassign command (#50) ([#50](https://github.com/lucabro81/CLI-monorepo/pull/50))
## [0.3.3](https://github.com/lucabro81/CLI-monorepo/compare/jira-v0.3.2...jira-v0.3.3) - 2026-07-24

### Fixed
- *(cli-fields)* cap --select-all response size to prevent context flooding
## [0.3.2](https://github.com/lucabro81/CLI-monorepo/compare/jira-v0.3.1...jira-v0.3.2) - 2026-07-23

### Fixed
- *(jira)* drop misleading --select example, point to per-command examples
## [0.3.1](https://github.com/lucabro81/CLI-monorepo/compare/jira-v0.3.0...jira-v0.3.1) - 2026-07-23

### Fixed
- *(jira)* correct --select path examples for issue get/search
## [0.3.0](https://github.com/lucabro81/CLI-monorepo/compare/jira-v0.2.1...jira-v0.3.0) - 2026-07-20

### Added
- *(jira)* add --stale-days flag to issue search

### Other
- *(jira)* document Service Account credentials as the recommended agent-only auth path
- *(jira)* fix e2e_smoke_doctor assertions to match current doctor schema
- list commands in README table of contents
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
