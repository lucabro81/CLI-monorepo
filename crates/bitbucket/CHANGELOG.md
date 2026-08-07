# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).
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
