# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).
## [0.5.0](https://github.com/lucabro81/CLI-monorepo/compare/google-chat-v0.4.0...google-chat-v0.5.0) - 2026-07-14

### Added
- *(google-chat)* require --message-filter or --allow-unfiltered on subscription create
## [0.4.0](https://github.com/lucabro81/CLI-monorepo/compare/google-chat-v0.3.0...google-chat-v0.4.0) - 2026-07-14

### Added
- *(google-chat)* add --message-filter to subscription create

### Other
- replace release-plz with git-cliff + cargo-release

## [Unreleased]

## [0.1.0](https://github.com/lucabro81/CLI-monorepo/releases/tag/google-chat-v0.1.0) - 2026-06-23

### Added

- *(google-chat)* implement messages send
- *(google-chat)* implement messages list
- *(google-chat)* implement spaces list
- *(google-chat)* implement init onboarding command
- *(google-chat)* implement doctor health check
- *(google-chat)* implement auth login (service account + 3LO)

### Fixed

- *(google-chat)* declare utf-8 charset on OAuth callback page

### Other

- *(google-chat)* add read-only e2e tests for spaces/messages list
- *(google-chat)* note service-account/DWD activation is pending, not abandoned
- scaffold google-chat crate docs and ADDENDUM
- scaffold google-chat crate skeleton
