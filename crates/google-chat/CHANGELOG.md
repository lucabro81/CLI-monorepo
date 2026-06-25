# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.2.0](https://github.com/lucabro81/CLI-monorepo/releases/tag/google-chat-v0.2.0) - 2026-06-25

### Added

- *(google-chat)* add subscription create/delete and listen for real-time Chat events ([#11](https://github.com/lucabro81/CLI-monorepo/pull/11))
- *(google-chat)* implement messages send
- *(google-chat)* implement messages list
- *(google-chat)* implement spaces list
- *(google-chat)* implement init onboarding command
- *(google-chat)* implement doctor health check
- *(google-chat)* implement auth login (service account + 3LO)

### Fixed

- *(google-chat)* bump version to 0.2.0 to work around release-plz registry check
- *(google-chat)* declare utf-8 charset on OAuth callback page

### Other

- release
- *(google-chat)* add read-only e2e tests for spaces/messages list
- *(google-chat)* note service-account/DWD activation is pending, not abandoned
- scaffold google-chat crate docs and ADDENDUM
- scaffold google-chat crate skeleton

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
