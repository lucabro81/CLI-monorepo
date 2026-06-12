#!/usr/bin/env bash
# Scaffolds a new CLI crate skeleton under crates/<name>.
#
# Usage: scripts/new-crate.sh <crate-name> "<Service description>"
#
# Creates only the parts that are identical across every crate in this
# workspace (Cargo.toml, fields.rs + tests, directory layout, workspace
# member, README registry row). Everything service-specific — auth.rs,
# client.rs, cli.rs, error.rs, context.rs, endpoints.rs, main.rs, CLAUDE.md,
# the crate README, and the add-<crate>-command skill/ADDENDUM — is left for
# the new-cli-crate skill to write after the auth-design and command-pool
# discussion. The placeholder files below intentionally do not compile yet.

set -euo pipefail

if [ $# -ne 2 ]; then
    echo "Usage: $0 <crate-name> \"<Service description>\"" >&2
    exit 1
fi

CRATE="$1"
DESCRIPTION="$2"
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
CRATE_DIR="$ROOT/crates/$CRATE"

if [ -e "$CRATE_DIR" ]; then
    echo "crates/$CRATE already exists, aborting." >&2
    exit 1
fi

echo "Creating crates/$CRATE ..."
mkdir -p "$CRATE_DIR/src/commands" "$CRATE_DIR/src/tests/commands" "$CRATE_DIR/.claude/skills/add-$CRATE-command"

# --- Cargo.toml ---------------------------------------------------------
cat > "$CRATE_DIR/Cargo.toml" <<EOF
[package]
name = "$CRATE"
version = "0.1.0"
edition = "2024"

[lints]
workspace = true

[dependencies]
clap = { version = "4", features = ["derive"] }
reqwest = { version = "0.12", features = ["blocking", "json"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
dirs = "5"
thiserror = "1"

[dev-dependencies]
tempfile = "3"
EOF

# --- fields.rs (generic, copied verbatim from bitbucket) ----------------
cp "$ROOT/crates/bitbucket/src/fields.rs" "$CRATE_DIR/src/fields.rs"
cp "$ROOT/crates/bitbucket/src/tests/fields_tests.rs" "$CRATE_DIR/src/tests/fields_tests.rs"

# --- Placeholder files, filled in by new-cli-crate skill -----------------
for f in auth client cli context endpoints error main; do
    cat > "$CRATE_DIR/src/$f.rs" <<EOF
//! TODO ($CRATE): written during the new-cli-crate skill's auth-design and
//! command-pool steps. $DESCRIPTION
EOF
done

cat > "$CRATE_DIR/src/commands/mod.rs" <<'EOF'
//! TODO: pub mod declarations for command handlers, added as commands land.
EOF

# --- Workspace member -----------------------------------------------------
sed -i.bak "s/members = \[\(.*\)\]/members = [\1, \"crates\/$CRATE\"]/" "$ROOT/Cargo.toml"
rm -f "$ROOT/Cargo.toml.bak"

echo "Done. Next: run the new-cli-crate skill to design auth, propose the"
echo "command pool, and write CLAUDE.md / README.md / ADDENDUM.md."
