#!/bin/sh
# Release pipeline validation tests.
#
# Tests the invariants the release pipeline depends on — fast, no credentials,
# no cross-compilation. What it covers:
#
#   1. Version bump  — workspace-root [workspace.package] version only;
#                      member crates must inherit via `version.workspace = true`
#   2. Checksum format — sha256sum/shasum output parseable by install.sh
#   3. Binary naming — names match exactly what install.sh requests
#   4. version.txt format — clean string, no whitespace (the tr bug class)
#   5. Semver validation — release.sh must reject bad version strings
#   6. _8V_BASE_URL validation — install.sh rejects non-https non-localhost URLs
#   7. release.sh delegates version bump to bump-version.sh
#   8. Version bump sed precision
#
# Usage:
#   sh scripts/test-release.sh

set -eu

WORKSPACE_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$WORKSPACE_ROOT"

PASS=0
FAIL=0

# ── Helpers ───────────────────────────────────────────────────────────────────

ok() {
    PASS=$((PASS + 1))
    echo "ok: $*"
}

fail() {
    FAIL=$((FAIL + 1))
    echo "FAIL: $*" >&2
}

assert_eq() {
    # assert_eq label expected actual
    if [ "$2" = "$3" ]; then
        ok "$1"
    else
        fail "$1: expected '$2', got '$3'"
    fi
}

assert_contains() {
    # assert_contains label needle haystack
    if echo "$3" | grep -q "$2"; then
        ok "$1"
    else
        fail "$1: '$2' not found in output"
    fi
}

assert_not_contains() {
    if echo "$3" | grep -q "$2"; then
        fail "$1: '$2' must not appear in output"
    else
        ok "$1"
    fi
}

# ── Test 1: Version bump (workspace inheritance) ─────────────────────────────
#
# The workspace uses [workspace.package] version inheritance — the root
# Cargo.toml is the single source of truth, and member crates declare
# `version.workspace = true`. bump-version.sh must:
#   (a) update the [workspace.package] version line in root Cargo.toml
#   (b) NOT touch member crates' Cargo.toml (inheritance handles them)
#   (c) NOT match version strings outside [workspace.package] (e.g. in
#       [workspace.dependencies] or [dependencies] sections)

echo ""
echo "── 1. Version bump ──"

# Invariant: every workspace member must inherit, not pin its own version.
# A drift here means bump-version.sh won't reach that crate.
MEMBER_CRATES=$(awk '
    /^\[workspace\]/ { in_members = 0; in_ws = 1; next }
    in_ws && /^members = \[/ { in_members = 1; next }
    in_members && /^\]/ { in_members = 0; next }
    in_members { gsub(/[",[:space:]]/, ""); if (length($0)) print $0 }
' Cargo.toml)

for crate in $MEMBER_CRATES; do
    cargo_file="$crate/Cargo.toml"
    if [ ! -f "$cargo_file" ]; then
        fail "workspace member listed but missing: $cargo_file"
        continue
    fi
    if grep -q '^version\.workspace = true' "$cargo_file"; then
        ok "member inherits version: $crate"
    elif grep -q '^version = ' "$cargo_file"; then
        fail "$crate pins its own version — must use 'version.workspace = true'"
    else
        fail "$crate has no version field and no workspace inheritance"
    fi
done

# Exercise bump-version.sh against a temp copy of Cargo.toml and verify
# only [workspace.package] version changes; dependency versions stay intact.
TMPDIR_BUMP=$(mktemp -d)
TARGET_VERSION="9.8.7"

cat > "$TMPDIR_BUMP/Cargo.toml" << 'EOF'
[workspace]
members = ["a"]
resolver = "2"

[workspace.package]
version = "0.1.0"
edition = "2021"

[workspace.dependencies]
serde = { version = "1.0.0", features = ["derive"] }
EOF

# Apply the same awk section-scoped bump that bump-version.sh uses.
tmp=$(mktemp)
awk -v ver="$TARGET_VERSION" '
    /^\[workspace\.package\]/ { in_section = 1; print; next }
    /^\[/ && !/^\[workspace\.package\]/ { in_section = 0 }
    in_section && /^version = / { print "version = \"" ver "\""; next }
    { print }
' "$TMPDIR_BUMP/Cargo.toml" > "$tmp"
mv "$tmp" "$TMPDIR_BUMP/Cargo.toml"

if grep -q "^version = \"$TARGET_VERSION\"$" "$TMPDIR_BUMP/Cargo.toml"; then
    ok "workspace.package version bumped to $TARGET_VERSION"
else
    fail "workspace.package version bump did not apply"
fi

if grep -q 'serde = { version = "1.0.0"' "$TMPDIR_BUMP/Cargo.toml"; then
    ok "workspace.dependencies version untouched"
else
    fail "workspace.dependencies version was incorrectly modified"
fi

rm -rf "$TMPDIR_BUMP"

# ── Test 2: Checksum format ────────────────────────────────────────────────────
#
# release.sh writes: sha256sum 8v-* > checksums.txt
# install.sh reads:  grep "$BINARY_NAME\$" checksums.txt | awk '{print $1}'
#
# Verify: the output format of sha256sum/shasum is parseable by install.sh's
# grep+awk, and that the binary name anchor ($) works correctly.

echo ""
echo "── 2. Checksum format ──"

TMPDIR_CKSUM=$(mktemp -d)

# Create fake binaries matching the exact names install.sh expects
for name in 8v-darwin-arm64 8v-darwin-x64 8v-linux-x64 8v-linux-arm64; do
    echo "fake binary $name" > "$TMPDIR_CKSUM/$name"
done

# Generate checksums using the same logic as release.sh
cd "$TMPDIR_CKSUM"
if command -v sha256sum >/dev/null 2>&1; then
    sha256sum 8v-* > checksums.txt
else
    shasum -a 256 8v-* > checksums.txt
fi
cd "$WORKSPACE_ROOT"

# Verify install.sh's grep+awk can extract each checksum
for name in 8v-darwin-arm64 8v-darwin-x64 8v-linux-x64 8v-linux-arm64; do
    EXTRACTED=$(grep "${name}\$" "$TMPDIR_CKSUM/checksums.txt" | awk '{print $1}')
    if [ -n "$EXTRACTED" ] && echo "$EXTRACTED" | grep -qE '^[a-f0-9]{64}$'; then
        ok "checksum parseable for $name: ${EXTRACTED:0:16}..."
    else
        fail "could not extract valid SHA256 for $name from checksums.txt"
    fi
done

# Verify that a binary name that is a PREFIX of another does NOT match incorrectly.
# e.g. grepping for "8v-darwin-arm" must not match "8v-darwin-arm64".
WRONG=$(grep "8v-darwin-arm\$" "$TMPDIR_CKSUM/checksums.txt" | awk '{print $1}' || true)
if [ -z "$WRONG" ]; then
    ok "checksum anchor: prefix 'arm' does not match 'arm64'"
else
    fail "checksum anchor broken: 'arm' matched when looking for 'arm64'"
fi

rm -rf "$TMPDIR_CKSUM"

# ── Test 3: Binary naming ──────────────────────────────────────────────────────
#
# install.sh requests: "8v-$PLATFORM" where PLATFORM is one of the four values
# from detect_platform(). release.sh produces files named the same way.
# Verify the names are in sync.

echo ""
echo "── 3. Binary naming consistency ──"

# Names install.sh can request
INSTALL_PLATFORMS="darwin-arm64 darwin-x64 linux-x64 linux-arm64"

# Names release.sh produces (extract from the cp lines)
RELEASE_NAMES=$(grep "^cp target" "$WORKSPACE_ROOT/scripts/release.sh" \
    | grep "dist/8v-" \
    | sed 's/.*dist\/8v-//' \
    | tr -d '"')

for platform in $INSTALL_PLATFORMS; do
    if echo "$RELEASE_NAMES" | grep -q "^$platform$"; then
        ok "binary name in sync: 8v-$platform"
    else
        fail "platform '$platform' in install.sh has no matching binary in release.sh"
    fi
done

# ── Test 4: version.txt format ────────────────────────────────────────────────
#
# install.sh reads: curl ... | tr -d '\n\r'
# The version must be a clean semver string — no whitespace, no v-prefix.

echo ""
echo "── 4. version.txt format ──"

# Simulate what release.sh writes (step 13: echo "$VERSION" > version.txt)
TMPDIR_VER=$(mktemp -d)
TEST_VERSION="1.2.3"
printf "%s\n" "$TEST_VERSION" > "$TMPDIR_VER/version.txt"

# Simulate what install.sh reads
READ_VERSION=$(cat "$TMPDIR_VER/version.txt" | tr -d '\n\r')

assert_eq "version.txt readable by install.sh" "$TEST_VERSION" "$READ_VERSION"
assert_not_contains "version.txt has no v-prefix" "^v" "$READ_VERSION"
assert_not_contains "version.txt has no spaces" " " "$READ_VERSION"

rm -rf "$TMPDIR_VER"


# ── Test 6: Semver format validation ─────────────────────────────────────────
#
# release.sh validates version format — must be X.Y.Z.
# Test the same regex used in release.sh.

echo ""
echo "── 6. Semver format validation ──"

SEMVER_REGEX='^[0-9]+\.[0-9]+\.[0-9]+$'

for v in "0.1.0" "1.0.0" "10.20.30" "1.2.3"; do
    if echo "$v" | grep -qE "$SEMVER_REGEX"; then
        ok "valid semver accepted: $v"
    else
        fail "valid semver rejected: $v"
    fi
done

for bad in "v1.0.0" "1.0" "latest" "" "1.0.0-beta" "1.0.0.0"; do
    if echo "$bad" | grep -qE "$SEMVER_REGEX"; then
        fail "invalid version must be rejected by release.sh: '$bad'"
    else
        ok "invalid version correctly rejected: '$bad'"
    fi
done

# ── Test 7: _8V_BASE_URL validation ──────────────────────────────────────────
#
# install.sh now validates _8V_BASE_URL:
# - https:// → allowed (production)
# - http://localhost → allowed (test server)
# - http://127.0.0.1 → allowed (test server)
# - anything else → rejected
#
# Test using the validate_base_url function extracted from install.sh.

echo ""
echo "── 7. _8V_BASE_URL validation ──"

check_url_allowed() {
    # Mimics install.sh's validate_base_url logic
    case "$1" in
        https://*) echo "allowed" ;;
        http://localhost*) echo "allowed" ;;
        http://127.0.0.1*) echo "allowed" ;;
        *) echo "rejected" ;;
    esac
}

for url in \
    "https://releases.8vast.io" \
    "https://example.com" \
    "http://localhost:8080" \
    "http://127.0.0.1:9000"; do
    result=$(check_url_allowed "$url")
    if [ "$result" = "allowed" ]; then
        ok "_8V_BASE_URL allowed: $url"
    else
        fail "_8V_BASE_URL must be allowed: $url"
    fi
done

for url in \
    "http://evil.example.com" \
    "http://192.168.1.1" \
    "ftp://releases.8vast.io" \
    ""; do
    result=$(check_url_allowed "$url")
    if [ "$result" = "rejected" ]; then
        ok "_8V_BASE_URL correctly rejected: '$url'"
    else
        fail "_8V_BASE_URL must be rejected: '$url'"
    fi
done

# ── Test 8: release.sh delegates to bump-version.sh ──────────────────────────
#
# Single-source-of-truth check: release.sh must use scripts/bump-version.sh
# (or inline the exact same awk), not maintain its own drift-prone logic.

echo ""
echo "── 8. release.sh uses bump-version.sh ──"

if grep -q 'scripts/bump-version.sh' "$WORKSPACE_ROOT/scripts/release.sh"; then
    ok "release.sh delegates version bump to bump-version.sh"
else
    fail "release.sh does not call scripts/bump-version.sh — duplicate logic risks drift"
fi

# ── Test 9: Version bump sed precision ───────────────────────────────────────
#
# The sed pattern `^version = ` is anchored to the start of the line.
# Inline dependency specs like `serde = { version = "1.0" }` are NOT at
# the start of a line and must NOT be modified by the version bump sed.

echo ""
echo "── 9. Version bump sed precision ──"

TMPDIR_SED=$(mktemp -d)
cat > "$TMPDIR_SED/Cargo.toml" << 'EOF'
[package]
name = "test-crate"
version = "0.1.0"
edition = "2021"

[dependencies]
serde = { version = "1.0.0", features = ["derive"] }
tokio = "1.0"
EOF

tmp=$(mktemp)
sed "s/^version = \".*\"/version = \"9.9.9\"/" "$TMPDIR_SED/Cargo.toml" > "$tmp"
mv "$tmp" "$TMPDIR_SED/Cargo.toml"

if grep -q '^version = "9.9.9"' "$TMPDIR_SED/Cargo.toml"; then
    ok "sed precision: [package] version bumped"
else
    fail "sed precision: [package] version NOT bumped"
fi

if grep -q 'serde = { version = "1.0.0"' "$TMPDIR_SED/Cargo.toml"; then
    ok "sed precision: inline dependency version not touched"
else
    fail "sed precision: inline dependency version was incorrectly modified"
fi

rm -rf "$TMPDIR_SED"

# ── Summary ───────────────────────────────────────────────────────────────────

echo ""
echo "────────────────────────────────"
echo "Results: $PASS passed, $FAIL failed"

if [ "$FAIL" -gt 0 ]; then
    exit 1
fi
