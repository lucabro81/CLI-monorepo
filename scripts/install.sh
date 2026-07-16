#!/usr/bin/env bash
# Install, update, or uninstall the jira/bitbucket/google-chat CLI binaries
# from this repo's GitHub Releases — no local clone, no cargo/Rust toolchain
# needed. Downloads the prebuilt binary for your platform straight from
# https://github.com/lucabro81/CLI-monorepo/releases.
#
# Can be run standalone, without cloning the repo, e.g.:
#   curl -fsSL https://raw.githubusercontent.com/lucabro81/CLI-monorepo/main/scripts/install.sh | bash -s install
#
# Usage:
#   install.sh [install|update|uninstall] [crate...]
#
# With no action, defaults to "install". With no crate names, applies to all
# three. install/update are the same operation (re-downloading overwrites).
#
# Env vars:
#   INSTALL_DIR   Where binaries are placed (default: $HOME/.local/bin)

set -euo pipefail

REPO="lucabro81/CLI-monorepo"
CRATES=(jira bitbucket google-chat)
INSTALL_DIR="${INSTALL_DIR:-$HOME/.local/bin}"

usage() {
  echo "Usage: $(basename "$0") [install|update|uninstall] [crate...]"
  echo "  crate: one or more of: ${CRATES[*]} (default: all)"
  exit 1
}

action="install"
if [ $# -ge 1 ]; then
  case "$1" in
  install | update | uninstall)
    action="$1"
    shift
    ;;
  esac
fi

targets=("$@")
[ ${#targets[@]} -gt 0 ] || targets=("${CRATES[@]}")

for crate in "${targets[@]}"; do
  case " ${CRATES[*]} " in
  *" $crate "*) ;;
  *)
    echo "Unknown crate: $crate (known: ${CRATES[*]})" >&2
    exit 1
    ;;
  esac
done

os="$(uname -s)"
arch="$(uname -m)"
case "$os-$arch" in
Linux-x86_64) suffix="linux-x86_64" ;;
Linux-aarch64 | Linux-arm64) suffix="linux-arm64" ;;
Darwin-arm64) suffix="macos-arm64" ;;
*)
  echo "Unsupported platform: $os $arch — prebuilt binaries only exist for linux-x86_64, linux-arm64, macos-arm64" >&2
  exit 1
  ;;
esac

# Highest semver version tagged "<crate>-v<version>" on the remote, without
# cloning the repo (git ls-remote only fetches refs, not history/objects).
latest_version() {
  local crate="$1"
  git ls-remote --tags "https://github.com/${REPO}.git" "${crate}-v*" |
    grep -o "refs/tags/${crate}-v[0-9][0-9.]*$" |
    sed "s#refs/tags/${crate}-v##" |
    sort -V |
    tail -1
}

case "$action" in
install | update)
  mkdir -p "$INSTALL_DIR"
  for crate in "${targets[@]}"; do
    version="$(latest_version "$crate")"
    if [ -z "$version" ]; then
      echo "No release found for $crate on GitHub, skipping" >&2
      continue
    fi
    tag="${crate}-v${version}"
    url="https://github.com/${REPO}/releases/download/${tag}/${crate}-${suffix}"
    dest="${INSTALL_DIR}/${crate}"
    echo "==> ${action}ing $crate ($tag) -> $dest"
    curl -fsSL "$url" -o "$dest"
    chmod +x "$dest"
  done
  echo
  echo "Done. Make sure $INSTALL_DIR is on your PATH:"
  echo "  export PATH=\"$INSTALL_DIR:\$PATH\""
  ;;
uninstall)
  for crate in "${targets[@]}"; do
    dest="${INSTALL_DIR}/${crate}"
    if [ -f "$dest" ]; then
      echo "==> removing $dest"
      rm -f "$dest"
    else
      echo "==> $crate not installed at $dest, skipping"
    fi
  done
  ;;
esac
