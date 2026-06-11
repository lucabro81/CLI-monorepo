# bitbucket

CLI for Bitbucket Cloud, designed to be driven by an LLM agent (output is JSON, errors are actionable).

## Status

`auth login` / `auth whoami` implemented. See [CLAUDE.md](CLAUDE.md) for architecture and the planned command list.

## Setup

1. In your Bitbucket workspace, go to Settings → OAuth consumers → Add consumer.
   Leave the callback URL empty (the `client_credentials` grant doesn't use it).
2. Write the consumer's Key/Secret to `~/.config/bitbucket-cli/app.json` (or
   `$XDG_CONFIG_HOME/bitbucket-cli/app.json`):

   ```json
   {"client_id": "...", "client_secret": "..."}
   ```

3. Run `bitbucket auth login` once. The access token is renewed automatically on
   subsequent commands when expired.

## Usage

```sh
bitbucket auth login
bitbucket auth whoami
bitbucket auth whoami --select uuid,display_name
```

## Development

```sh
cargo build -p bitbucket
cargo run -p bitbucket -- --help
```
