# CLI monorepo

A Cargo workspace of CLI tools, one per external service, designed to be driven by an LLM agent in place of an MCP server. See [CLAUDE.md](CLAUDE.md) for the design principles and conventions every crate follows.

## CLIs

| Crate | Service | Status |
|-------|---------|--------|
| [jira](crates/jira/README.md) | Jira Cloud (issues, comments, transitions, search) | Implemented — see crate README for setup and commands |
| [bitbucket](crates/bitbucket/README.md) | Bitbucket Cloud (PRs, repos) | init/doctor/auth/repo get/list/create implemented — see crate CLAUDE.md for planned commands |
| [google-chat](crates/google-chat/README.md) | Google Chat (Workspace) | auth implemented — see crate CLAUDE.md for planned commands |
| [atlassian-admin](crates/atlassian-admin/README.md) | Atlassian Organization Admin API (account_id → email/profile lookup) | Scaffolded, not yet implemented — see crate CLAUDE.md |

## Install prebuilt binaries

No clone, no Rust toolchain needed — downloads the release binary for your platform (linux-x86_64, linux-arm64, macos-arm64) straight from GitHub Releases:

```sh
curl -fsSL https://raw.githubusercontent.com/lucabro81/CLI-monorepo/main/scripts/install.sh | bash -s install
```

Installs to `$HOME/.local/bin` by default (override with `INSTALL_DIR=...`). Same command with `update` re-downloads the latest version; `uninstall` removes them. Pass crate names to target just one, e.g. `... | bash -s install jira`.

## Development

```sh
cargo build                  # whole workspace
cargo build -p <crate>
cargo test -p <crate>
cargo clippy -p <crate>      # must pass with zero warnings
cargo run -p <crate> -- --help
```

See each crate's own `README.md` for service-specific setup (OAuth apps, credentials, etc.) and `CLAUDE.md` for its architecture.
