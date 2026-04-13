#!/bin/bash
set -euo pipefail

# Bump version script for 8v
# Updates version in all workspace Cargo.toml files
# Usage: ./scripts/bump-version.sh 0.2.0

VERSION="${1:-}"

if [ -z "$VERSION" ]; then
    echo "Usage: ./scripts/bump-version.sh 0.2.0" >&2
    exit 1
fi

# Get workspace root
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WORKSPACE_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
cd "$WORKSPACE_ROOT"

# Update all 8 Cargo.toml files in workspace crates
for cargo_file in \
    Cargo.toml \
    o8v/Cargo.toml \
    o8v-core/Cargo.toml \
    o8v-fs/Cargo.toml \
    o8v-process/Cargo.toml \
    o8v-testkit/Cargo.toml; do

    if [ -f "$cargo_file" ]; then
        sed -i '' "s/^version = \".*\"/version = \"$VERSION\"/" "$cargo_file"
    fi
done

# Regenerate Cargo.lock
cargo check -p o8v > /dev/null 2>&1

echo "✓ Bumped to $VERSION"
