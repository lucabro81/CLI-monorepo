# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).
## [2.3.2](https://github.com/lucabro81/CLI-monorepo/compare/confluence-v2.3.1...confluence-v2.3.2) - 2026-10-09

### Fixed
- *(confluence)* renew a token answered 401 and retry, exit 3 when the person's login is gone (#240)
- *(confluence)* auth whoami exits 3 when a revoked login cannot be renewed (#240)

### Other
- *(confluence)* a revoked token renews after a 401, exit 3 when the login is gone (#240)
- *(confluence)* client retry after a 401 and doctor's revoked-token check against a local server (#240)
- *(confluence)* doctor --help names the renewal after a 401 (#240)
- Merge pull request #241 from lucabro81/issue240 ([#241](https://github.com/lucabro81/CLI-monorepo/pull/241))
- Release
- Merge pull request #245 from lucabro81/release/confluence ([#245](https://github.com/lucabro81/CLI-monorepo/pull/245))
## [2.3.2](https://github.com/lucabro81/CLI-monorepo/compare/confluence-v2.3.1...confluence-v2.3.2) - 2026-10-09

### Fixed
- *(confluence)* renew a token answered 401 and retry, exit 3 when the person's login is gone (#240)
- *(confluence)* auth whoami exits 3 when a revoked login cannot be renewed (#240)

### Other
- *(confluence)* a revoked token renews after a 401, exit 3 when the login is gone (#240)
- *(confluence)* client retry after a 401 and doctor's revoked-token check against a local server (#240)
- *(confluence)* doctor --help names the renewal after a 401 (#240)
- Merge pull request #241 from lucabro81/issue240 ([#241](https://github.com/lucabro81/CLI-monorepo/pull/241))
## [2.3.1](https://github.com/lucabro81/CLI-monorepo/compare/confluence-v2.3.0...confluence-v2.3.1) - 2026-10-07

### Fixed
- *(confluence)* an empty init prompt answer writes nothing

### Other
- review fixes for #218 (help texts, oauth-user-login users, error wording)
- Merge pull request #219 from lucabro81/issue218 ([#219](https://github.com/lucabro81/CLI-monorepo/pull/219))
## [2.3.0](https://github.com/lucabro81/CLI-monorepo/compare/confluence-v2.2.0...confluence-v2.3.0) - 2026-10-07

### Added
- *(confluence)* init --user-app sets up the people's app without logging anyone in
- *(confluence)* read the client secret hidden on a terminal

### Other
- *(confluence)* init --user-app
- init --user-app in CLAUDE.md files (#195)
- --user-app review fixes (#195)
- Merge pull request #207 from lucabro81/issue195 ([#207](https://github.com/lucabro81/CLI-monorepo/pull/207))
- *(confluence)* the client secret prompt is hidden on a terminal
- Merge pull request #212 from lucabro81/issue196 ([#212](https://github.com/lucabro81/CLI-monorepo/pull/212))
## [2.2.0](https://github.com/lucabro81/CLI-monorepo/compare/confluence-v2.1.0...confluence-v2.2.0) - 2026-10-07

### Added
- *(confluence)* exit code 3 when the selected identity needs a new login

### Other
- *(confluence)* document exit code 3 for a missing or expired login
- exit code 3 for a missing or expired login (#194)
- *(confluence)* invalid_client exits 1
- exit code docs fixes from review (#194)
- Merge pull request #197 from lucabro81/issue194 ([#197](https://github.com/lucabro81/CLI-monorepo/pull/197))
## [2.1.0](https://github.com/lucabro81/CLI-monorepo/compare/confluence-v2.0.0...confluence-v2.1.0) - 2026-10-07

### Added
- *(confluence)* accept ':' in --user ids

### Other
- Merge pull request #185 from lucabro81/issue184 ([#185](https://github.com/lucabro81/CLI-monorepo/pull/185))
## [2.0.0](https://github.com/lucabro81/CLI-monorepo/compare/confluence-v1.0.0...confluence-v2.0.0) - 2026-10-06

### Added
- *(confluence)* --user <id> selects a person, one credentials folder each, auth logout

### Fixed
- *(confluence)* name the identity in the nothing-to-log-out error
- *(confluence)* report an unwritable credentials file instead of asking to log in again

### Other
- *(confluence)* document --user <id>, per-person folders and auth logout (#175)
- *(confluence)* explain the .lock file next to the credentials (#175)
- Merge pull request #176 from lucabro81/issue175 ([#176](https://github.com/lucabro81/CLI-monorepo/pull/176))
## [1.0.0](https://github.com/lucabro81/CLI-monorepo/compare/confluence-v0.4.0...confluence-v1.0.0) - 2026-10-06

### Added
- *(atlassian-auth)* add Identity and sectioned AppConfig
- *(confluence)* keep service and human credentials side by side, pick one per call with --user

### Fixed
- *(atlassian-auth)* refuse credentials that don't match the identity they are loaded as
- *(confluence)* say so when init replaces an old single-identity app.json

### Other
- *(atlassian-auth)* drop the flat app.json API now that every caller uses AppConfig
- document side-by-side service and user identities
- fix stale references after the identity split
- *(confluence)* remote login step 2 prints auth whoami --user
- Merge pull request #166 from lucabro81/issue164 ([#166](https://github.com/lucabro81/CLI-monorepo/pull/166))
## [0.4.0](https://github.com/lucabro81/CLI-monorepo/compare/confluence-v0.3.0...confluence-v0.4.0) - 2026-10-05

### Added
- *(confluence)* add auth login --user --remote, a two-step login for someone elsewhere

### Fixed
- *(confluence)* end every remote login error with how to restart it

### Other
- remove resolved entries from BACKLOG.md
- Merge pull request #65 from lucabro81/chore/backlog-cleanup ([#65](https://github.com/lucabro81/CLI-monorepo/pull/65))
- *(confluence)* point PKCE and the callback listener to oauth-user-login
- Merge pull request #147 from lucabro81/issue143 ([#147](https://github.com/lucabro81/CLI-monorepo/pull/147))
- *(oauth-user-login)* make the pending login's redirect URI optional
- *(confluence)* say in doctor --help that pending_login is informational
- Merge pull request #154 from lucabro81/issue146 ([#154](https://github.com/lucabro81/CLI-monorepo/pull/154))
## [0.3.0](https://github.com/lucabro81/CLI-monorepo/compare/confluence-v0.2.0...confluence-v0.3.0) - 2026-08-03

### Added
- *(confluence)* add page delete, template update, template delete

### Other
- Merge pull request #63 from lucabro81/feat/confluence-page-template-delete-update ([#63](https://github.com/lucabro81/CLI-monorepo/pull/63))
## [0.2.0](https://github.com/lucabro81/CLI-monorepo/compare/confluence-v0.1.0...confluence-v0.2.0) - 2026-08-03

### Added
- *(confluence)* add template create/list commands

### Fixed
- *(confluence)* rename page create's --template-file to --body-file

### Other
- Release
- Merge pull request #57 from lucabro81/release/confluence ([#57](https://github.com/lucabro81/CLI-monorepo/pull/57))
- Merge pull request #60 from lucabro81/fix/confluence-body-file-flag ([#60](https://github.com/lucabro81/CLI-monorepo/pull/60))
## [0.1.0](https://github.com/lucabro81/CLI-monorepo/releases/tag/confluence-v0.1.0) - 2026-08-03

### Added
- *(confluence)* add Confluence Cloud CLI crate

### Fixed
- *(atlassian-auth)* union scopes across same-cloud_id accessible-resources entries

### Other
- *(confluence)* make Service-Account-skips-init warning visible up front
- Merge pull request #56 from lucabro81/feat/confluence-crate ([#56](https://github.com/lucabro81/CLI-monorepo/pull/56))
