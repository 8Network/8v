#!/bin/bash
set -euo pipefail

# Bump version script for 8v.
# Workspace uses [workspace.package] version inheritance — member crates
# declare `version.workspace = true`, so the root Cargo.toml is the single
# source of truth.
#
# Usage: ./scripts/bump-version.sh 0.2.0

VERSION="${1:-}"

if [ -z "$VERSION" ]; then
    echo "Usage: ./scripts/bump-version.sh 0.2.0" >&2
    exit 1
fi

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WORKSPACE_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
cd "$WORKSPACE_ROOT"

# Bump the workspace-wide version. The regex anchors on the [workspace.package]
# section header to avoid touching dependency version strings that appear in
# [workspace.dependencies].
#
# Portable sed: mktemp + mv, so this works on both BSD (macOS) and GNU sed.
tmp=$(mktemp)
awk -v ver="$VERSION" '
    /^\[workspace\.package\]/ { in_section = 1; print; next }
    /^\[/ && !/^\[workspace\.package\]/ { in_section = 0 }
    in_section && /^version = / { print "version = \"" ver "\""; next }
    { print }
' Cargo.toml > "$tmp"
mv "$tmp" Cargo.toml

# Verify the bump actually happened — catch regex drift loudly.
if ! grep -q "^version = \"$VERSION\"$" Cargo.toml; then
    echo "✗ Version bump did not apply to root Cargo.toml" >&2
    exit 1
fi

# Regenerate Cargo.lock so downstream tools see the new version.
cargo check --workspace > /dev/null 2>&1

echo "✓ Bumped [workspace.package] version to $VERSION"
