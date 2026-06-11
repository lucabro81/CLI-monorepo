# CLI monorepo

A Cargo workspace of CLI tools, one per external service, designed to be driven by an LLM agent in place of an MCP server. See [CLAUDE.md](CLAUDE.md) for the design principles and conventions every crate follows.

## CLIs

| Crate | Service | Status |
|-------|---------|--------|
| [jira](crates/jira/README.md) | Jira Cloud (issues, comments, transitions, search) | Implemented — see crate README for setup and commands |
| [bitbucket](crates/bitbucket/README.md) | Bitbucket Cloud (PRs, repos) | Auth implemented (login/whoami) — see crate CLAUDE.md for planned commands |

## Development

```sh
cargo build                  # whole workspace
cargo build -p <crate>
cargo test -p <crate>
cargo clippy -p <crate>      # must pass with zero warnings
cargo run -p <crate> -- --help
```

See each crate's own `README.md` for service-specific setup (OAuth apps, credentials, etc.) and `CLAUDE.md` for its architecture.
