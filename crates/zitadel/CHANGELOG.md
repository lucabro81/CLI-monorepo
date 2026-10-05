# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).
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
