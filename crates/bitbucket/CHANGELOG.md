# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).
## [2.3.1](https://github.com/lucabro81/CLI-monorepo/compare/bitbucket-v2.3.0...bitbucket-v2.3.1) - 2026-10-07

### Fixed
- *(bitbucket)* an empty init prompt answer writes nothing

### Other
- review fixes for #218 (help texts, oauth-user-login users, error wording)
- Merge pull request #219 from lucabro81/issue218 ([#219](https://github.com/lucabro81/CLI-monorepo/pull/219))
## [2.3.0](https://github.com/lucabro81/CLI-monorepo/compare/bitbucket-v2.2.0...bitbucket-v2.3.0) - 2026-10-07

### Added
- *(bitbucket)* init --user-app sets up the people's app without logging anyone in
- *(bitbucket)* read the client secret hidden on a terminal

### Other
- *(bitbucket)* init --user-app
- init --user-app in CLAUDE.md files (#195)
- --user-app review fixes (#195)
- Merge pull request #207 from lucabro81/issue195 ([#207](https://github.com/lucabro81/CLI-monorepo/pull/207))
- *(bitbucket)* the client secret prompt is hidden on a terminal
- Merge pull request #212 from lucabro81/issue196 ([#212](https://github.com/lucabro81/CLI-monorepo/pull/212))
## [2.2.0](https://github.com/lucabro81/CLI-monorepo/compare/bitbucket-v2.1.0...bitbucket-v2.2.0) - 2026-10-07

### Added
- *(bitbucket)* exit code 3 when the selected identity needs a new login

### Fixed
- *(bitbucket)* invalid_client is not a refused grant

### Other
- *(bitbucket)* document exit code 3 for a missing or expired login
- exit code 3 for a missing or expired login (#194)
- exit code docs fixes from review (#194)
- Merge pull request #197 from lucabro81/issue194 ([#197](https://github.com/lucabro81/CLI-monorepo/pull/197))
## [2.1.0](https://github.com/lucabro81/CLI-monorepo/compare/bitbucket-v2.0.0...bitbucket-v2.1.0) - 2026-10-07

### Added
- *(bitbucket)* accept ':' in --user ids

### Other
- Merge pull request #185 from lucabro81/issue184 ([#185](https://github.com/lucabro81/CLI-monorepo/pull/185))
## [2.0.0](https://github.com/lucabro81/CLI-monorepo/compare/bitbucket-v1.0.0...bitbucket-v2.0.0) - 2026-10-06

### Added
- *(bitbucket)* --user <id> selects a person, one credentials folder each, auth logout

### Fixed
- *(bitbucket)* browser login retry hints name --user <USER_ID>
- *(bitbucket)* name the identity in the nothing-to-log-out error
- *(bitbucket)* report an unwritable credentials file instead of asking to log in again

### Other
- *(bitbucket)* document --user <id>, per-person folders and auth logout (#175)
- *(bitbucket)* explain the .lock file next to the credentials (#175)
- Merge pull request #176 from lucabro81/issue175 ([#176](https://github.com/lucabro81/CLI-monorepo/pull/176))
## [1.0.0](https://github.com/lucabro81/CLI-monorepo/compare/bitbucket-v0.11.0...bitbucket-v1.0.0) - 2026-10-06

### Added
- *(bitbucket)* keep app and human credentials side by side, pick one per call with --user

### Fixed
- *(bitbucket)* refuse credentials that don't match the identity they are loaded as
- *(bitbucket)* say so when init replaces an old single-identity app.json

### Other
- document side-by-side service and user identities
- fix stale references after the identity split
- *(bitbucket)* remote login step 2 prints auth whoami --user
- *(bitbucket)* document workspace members and complete README status list
- *(bitbucket)* placeholder workspace in examples, document mandatory --select
- *(bitbucket)* placeholder reviewer uuid in examples
- Merge pull request #166 from lucabro81/issue164 ([#166](https://github.com/lucabro81/CLI-monorepo/pull/166))
## [0.11.0](https://github.com/lucabro81/CLI-monorepo/compare/bitbucket-v0.10.0...bitbucket-v0.11.0) - 2026-10-05

### Added
- *(bitbucket)* add auth login --user --remote, a two-step login for someone elsewhere

### Fixed
- *(bitbucket)* end every remote login error with how to restart it
- *(bitbucket)* never copy token response bodies into errors

### Other
- *(bitbucket)* use oauth-user-login for state and the login callback
- Merge pull request #147 from lucabro81/issue143 ([#147](https://github.com/lucabro81/CLI-monorepo/pull/147))
- *(bitbucket)* document the two-step remote login
- *(bitbucket)* say in doctor --help that pending_login is informational
- Merge pull request #154 from lucabro81/issue146 ([#154](https://github.com/lucabro81/CLI-monorepo/pull/154))
## [0.10.0](https://github.com/lucabro81/CLI-monorepo/compare/bitbucket-v0.9.0...bitbucket-v0.10.0) - 2026-10-02

### Added
- *(bitbucket)* add auth login --user (authorization_code grant)

### Other
- *(bitbucket)* document auth login --user and the callback URL
- Merge pull request #135 from lucabro81/issue96 ([#135](https://github.com/lucabro81/CLI-monorepo/pull/135))
## [0.9.0](https://github.com/lucabro81/CLI-monorepo/compare/bitbucket-v0.8.0...bitbucket-v0.9.0) - 2026-09-29

### Added
- *(bitbucket)* reply to a pull request comment with pr comment --parent

### Other
- *(bitbucket)* document pr comment --parent
- Merge pull request #130 from lucabro81/issue129 ([#130](https://github.com/lucabro81/CLI-monorepo/pull/130))
## [0.8.0](https://github.com/lucabro81/CLI-monorepo/compare/bitbucket-v0.7.0...bitbucket-v0.8.0) - 2026-09-28

### Added
- *(bitbucket)* add pr list-comments and pr update-comment

### Other
- *(bitbucket)* document pr create --reviewers in README
- *(bitbucket)* document pr list-comments and pr update-comment
- *(bitbucket)* reject non-numeric comment id; clarify build_comment_body doc
- Merge pull request #127 from lucabro81/issue125 ([#127](https://github.com/lucabro81/CLI-monorepo/pull/127))
## [0.7.0](https://github.com/lucabro81/CLI-monorepo/compare/bitbucket-v0.6.0...bitbucket-v0.7.0) - 2026-09-28

### Added
- *(bitbucket)* support draft pull requests in pr create and pr update

### Other
- *(bitbucket)* add branch create/suggest-name to README ToC
- *(bitbucket)* document pr create/update draft flags
- Merge pull request #123 from lucabro81/issue121 ([#123](https://github.com/lucabro81/CLI-monorepo/pull/123))
## [0.6.0](https://github.com/lucabro81/CLI-monorepo/compare/bitbucket-v0.5.0...bitbucket-v0.6.0) - 2026-08-07

### Added
- *(bitbucket)* add branch suggest-name command

### Other
- Merge pull request #119 from lucabro81/issue118 ([#119](https://github.com/lucabro81/CLI-monorepo/pull/119))
## [0.5.0](https://github.com/lucabro81/CLI-monorepo/compare/bitbucket-v0.4.0...bitbucket-v0.5.0) - 2026-08-07

### Added
- *(bitbucket)* add branch create command

### Other
- Merge pull request #115 from lucabro81/issue114 ([#115](https://github.com/lucabro81/CLI-monorepo/pull/115))
## [0.4.0](https://github.com/lucabro81/CLI-monorepo/compare/bitbucket-v0.3.0...bitbucket-v0.4.0) - 2026-08-05

### Added
- *(bitbucket)* add pr update command

### Other
- Merge pull request #101 from lucabro81/issue100 ([#101](https://github.com/lucabro81/CLI-monorepo/pull/101))
## [0.3.0](https://github.com/lucabro81/CLI-monorepo/compare/bitbucket-v0.2.3...bitbucket-v0.3.0) - 2026-08-05

### Added
- *(bitbucket)* add pr create --reviewers and workspace members lookup

### Other
- migrate BACKLOG.md to GitHub issues
- Merge pull request #97 from lucabro81/issue86 ([#97](https://github.com/lucabro81/CLI-monorepo/pull/97))
## [0.2.3](https://github.com/lucabro81/CLI-monorepo/compare/bitbucket-v0.2.2...bitbucket-v0.2.3) - 2026-07-24

### Fixed
- *(cli-fields)* cap --select-all response size to prevent context flooding
## [0.2.2](https://github.com/lucabro81/CLI-monorepo/compare/bitbucket-v0.2.1...bitbucket-v0.2.2) - 2026-07-22

### Fixed
- *(bitbucket)* include raw response body in token exchange errors
## [0.2.1](https://github.com/lucabro81/CLI-monorepo/compare/bitbucket-v0.2.0...bitbucket-v0.2.1) - 2026-07-22

### Fixed
- *(bitbucket)* parse OAuth token response's "scope" field, not "scopes"

### Other
- replace release-plz with git-cliff + cargo-release
- list commands in README table of contents

## [Unreleased]

## [0.1.0](https://github.com/lucabro81/CLI-monorepo/releases/tag/bitbucket-v0.1.0) - 2026-06-21

### Added

- add new-cli-crate skill and scaffold script
- *(bitbucket)* add pr diff command
- *(bitbucket)* add repo delete command
- *(bitbucket)* add branch list command
- *(bitbucket)* add pr merge command
- *(bitbucket)* add pr approve, unapprove, decline commands
- *(bitbucket)* add pr comment command
- *(bitbucket)* add pr create command
- *(bitbucket)* add pr get command
- *(bitbucket)* add pr list command
- *(bitbucket)* add repo create command
- *(bitbucket)* add repo list command
- *(bitbucket)* add init and doctor commands with scope-based permissions check
- *(bitbucket)* add repo get command
- *(bitbucket)* add bitbucket crate with OAuth client_credentials auth

### Other

- align root structure convention with actual crate layout
- document two-level test split in jira/bitbucket CLAUDE.md
- *(bitbucket)* move test files into src/tests/
- clarify addendum step-numbering convention for agents
- align e2e-test addendum structure across jira and bitbucket
- *(bitbucket)* add e2e pr lifecycle test
- trim addendum duplication with per-crate CLAUDE.md
- unify add-jira-command/add-bitbucket-command into shared skill
- *(bitbucket)* move split_repository to context for sharing
- *(bitbucket)* rewrite README to match jira's style, add command skill
