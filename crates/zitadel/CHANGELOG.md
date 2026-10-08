# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).
## [2.5.0](https://github.com/lucabro81/CLI-monorepo/compare/zitadel-v2.4.0...zitadel-v2.5.0) - 2026-10-08

### Added
- *(zitadel)* user authorizations lists a user's project roles

### Other
- *(zitadel)* user authorizations and the read-only service-user role (#229)
- *(zitadel)* e2e shows why user authorizations checks the user first (#229)
- Merge pull request #234 from lucabro81/issue229 ([#234](https://github.com/lucabro81/CLI-monorepo/pull/234))
- Release
- Merge pull request #235 from lucabro81/release/zitadel ([#235](https://github.com/lucabro81/CLI-monorepo/pull/235))
## [2.5.0](https://github.com/lucabro81/CLI-monorepo/compare/zitadel-v2.4.0...zitadel-v2.5.0) - 2026-10-08

### Added
- *(zitadel)* user authorizations lists a user's project roles

### Other
- *(zitadel)* user authorizations and the read-only service-user role (#229)
- *(zitadel)* e2e shows why user authorizations checks the user first (#229)
- Merge pull request #234 from lucabro81/issue229 ([#234](https://github.com/lucabro81/CLI-monorepo/pull/234))
## [2.4.0](https://github.com/lucabro81/CLI-monorepo/compare/zitadel-v2.3.0...zitadel-v2.4.0) - 2026-10-08

### Added
- *(zitadel)* init asks for what the flags didn't give, the key JSON hidden

### Other
- *(zitadel)* interactive init (#228)
- *(zitadel)* init review fixes (#228)
- *(zitadel)* init --help and README review fixes (#228)
- *(zitadel)* README setup shows the interactive init (#228)
- *(zitadel)* init errors in README error design (#228)
- Merge pull request #231 from lucabro81/issue228 ([#231](https://github.com/lucabro81/CLI-monorepo/pull/231))
## [2.3.0](https://github.com/lucabro81/CLI-monorepo/compare/zitadel-v2.2.0...zitadel-v2.3.0) - 2026-10-07

### Added
- *(zitadel)* init --user-app sets up the Native app without logging anyone in

### Other
- *(zitadel)* init --user-app
- init --user-app in CLAUDE.md files (#195)
- --user-app review fixes (#195)
- Merge pull request #207 from lucabro81/issue195 ([#207](https://github.com/lucabro81/CLI-monorepo/pull/207))
## [2.2.0](https://github.com/lucabro81/CLI-monorepo/compare/zitadel-v2.1.0...zitadel-v2.2.0) - 2026-10-07

### Added
- *(zitadel)* exit code 3 when the selected identity needs a new login

### Fixed
- *(zitadel)* invalid_client is not a refused grant

### Other
- *(zitadel)* document exit code 3 for a missing or expired login
- *(zitadel)* exit code 3 in crate CLAUDE.md
- exit code docs fixes from review (#194)
- Merge pull request #197 from lucabro81/issue194 ([#197](https://github.com/lucabro81/CLI-monorepo/pull/197))
## [2.1.0](https://github.com/lucabro81/CLI-monorepo/compare/zitadel-v2.0.0...zitadel-v2.1.0) - 2026-10-07

### Added
- *(zitadel)* accept ':' in --user ids

### Other
- Merge pull request #185 from lucabro81/issue184 ([#185](https://github.com/lucabro81/CLI-monorepo/pull/185))
## [2.0.0](https://github.com/lucabro81/CLI-monorepo/compare/zitadel-v1.0.0...zitadel-v2.0.0) - 2026-10-06

### Added
- *(zitadel)* --user <id> selects a person, one credentials folder each, auth logout

### Fixed
- *(zitadel)* name the identity in the nothing-to-log-out error

### Other
- *(zitadel)* document --user <id>, per-person folders and auth logout (#175)
- *(zitadel)* explain the .lock file next to the credentials (#175)
- Merge pull request #176 from lucabro81/issue175 ([#176](https://github.com/lucabro81/CLI-monorepo/pull/176))
## [1.0.0](https://github.com/lucabro81/CLI-monorepo/compare/zitadel-v0.2.0...zitadel-v1.0.0) - 2026-10-06

### Added
- *(zitadel)* keep service user and human credentials side by side, pick one per call with --user

### Fixed
- *(zitadel)* refuse credentials that don't match the identity they are loaded as

### Other
- document side-by-side service and user identities
- fix stale references after the identity split
- *(zitadel)* remote login step 2 prints auth whoami --user
- Merge pull request #166 from lucabro81/issue164 ([#166](https://github.com/lucabro81/CLI-monorepo/pull/166))
## [0.2.0](https://github.com/lucabro81/CLI-monorepo/compare/zitadel-v0.1.0...zitadel-v0.2.0) - 2026-10-05

### Added
- *(zitadel)* add auth login --user --remote, a two-step login for someone elsewhere

### Fixed
- *(zitadel)* print the login retry command once on callback errors
- *(zitadel)* end every remote login error with how to restart it

### Other
- Release
- Merge pull request #145 from lucabro81/release/zitadel ([#145](https://github.com/lucabro81/CLI-monorepo/pull/145))
- *(zitadel)* use oauth-user-login for PKCE and the login callback
- *(zitadel)* document the two-step remote login
- Merge pull request #147 from lucabro81/issue143 ([#147](https://github.com/lucabro81/CLI-monorepo/pull/147))
- *(oauth-user-login)* make the pending login's redirect URI optional
- *(zitadel)* say in doctor --help that pending_login is informational
- Merge pull request #154 from lucabro81/issue146 ([#154](https://github.com/lucabro81/CLI-monorepo/pull/154))
## [0.1.0](https://github.com/lucabro81/CLI-monorepo/releases/tag/zitadel-v0.1.0) - 2026-10-05

### Added
- *(zitadel)* add auth login with service user private key JWT
- *(zitadel)* add auth whoami and authenticated API client
- *(zitadel)* add doctor with identity and administrator-role report
- *(zitadel)* add init for flag-driven onboarding
- *(zitadel)* add user search
- *(zitadel)* add user get
- *(zitadel)* add organization list
- *(zitadel)* add project list
- *(zitadel)* add auth login --user (authorization code + PKCE)

### Fixed
- *(zitadel)* report a failed credentials save as a save error, not a re-login
- *(zitadel)* drop the previous instance's token when init changes the URL
- *(zitadel)* make login and generic API errors say what to do next
- *(zitadel)* write credentials.json with owner-only permissions

### Other
- scaffold zitadel crate docs and ADDENDUM
- *(zitadel)* document init, doctor, auth login and auth whoami usage
- *(zitadel)* add read-only e2e suite against a real instance
- wire zitadel into release pipeline
- *(zitadel)* cover error branches and assert exact errors
- *(zitadel)* correct ADDENDUM scope location and document both v2 API shapes
- *(zitadel)* mark the initial command set as complete
- Merge pull request #144 from lucabro81/issue142 ([#144](https://github.com/lucabro81/CLI-monorepo/pull/144))
