# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.2.0](https://github.com/lucabro81/CLI-monorepo/releases/tag/bitbucket-v0.2.0) - 2026-07-11

### Added

- *(bitbucket)* require --select or --select-all before printing full JSON
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

### Fixed

- *(bitbucket)* bump version to 0.2.0 to work around release-plz registry check

### Other

- Update README.md
- Update README.md
- release v0.1.0
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
