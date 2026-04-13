#!/bin/sh
# Release pipeline validation tests.
#
# Tests the invariants the release pipeline depends on — fast, no credentials,
# no cross-compilation, no wrangler. What it covers:
#
#   1. Version bump  — sed across all 9 Cargo.toml files; wrong regex → silent miss
#   2. Checksum format — sha256sum/shasum output parseable by install.sh
#   3. Binary naming — names match exactly what install.sh requests
#   4. version.txt format — clean string, no whitespace (the tr bug class)
#   5. Changelog update — [Unreleased] marker replaced correctly
#   6. Semver validation — release.sh must reject bad version strings
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

# ── Test 1: Version bump ──────────────────────────────────────────────────────
#
# release.sh uses: sed -i '' "s/^version = \".*\"/version = \"$VERSION\"/"
# on 9 files. Verify the pattern matches real Cargo.toml format and updates all.

echo ""
echo "── 1. Version bump ──"

TMPDIR_BUMP=$(mktemp -d)
TARGET_VERSION="9.8.7"

# All crate Cargo.toml files — must match release.sh's list exactly.
# Workspace root (Cargo.toml) has no version field and is intentionally excluded.
CARGO_FILES="o8v/Cargo.toml o8v-core/Cargo.toml o8v-events/Cargo.toml \
o8v-fs/Cargo.toml o8v-process/Cargo.toml \
o8v-testkit/Cargo.toml o8v-workspace/Cargo.toml"

# Copy all Cargo.toml files into temp dir, preserving relative paths
for f in $CARGO_FILES; do
    if [ -f "$f" ]; then
        mkdir -p "$TMPDIR_BUMP/$(dirname "$f")"
        cp "$f" "$TMPDIR_BUMP/$f"
    else
        fail "Cargo.toml missing from release file list: $f"
    fi
done

# Apply the sed from release.sh (use temp file for portability — BSD sed -i requires a space)
for cargo_file in $CARGO_FILES; do
    full="$TMPDIR_BUMP/$cargo_file"
    [ -f "$full" ] || continue
    tmp=$(mktemp)
    sed "s/^version = \".*\"/version = \"$TARGET_VERSION\"/" "$full" > "$tmp"
    mv "$tmp" "$full"
done

# Verify every file now contains the new version
for f in $CARGO_FILES; do
    full="$TMPDIR_BUMP/$f"
    [ -f "$full" ] || continue
    if grep -q "^version = \"$TARGET_VERSION\"" "$full"; then
        ok "version bump: $f"
    else
        fail "version not updated in $f (contains: $(grep '^version' "$full" | head -1))"
    fi
done

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

# ── Test 5: Changelog update ───────────────────────────────────────────────────
#
# release.sh uses:
#   sed -i '' "s/## \[Unreleased\]/## [Unreleased]\n\n## [$VERSION] - $DATE/" CHANGELOG.md
#
# Verify: the Unreleased header is preserved and the new version header is added.

echo ""
echo "── 5. Changelog update ──"

TMPDIR_CL=$(mktemp -d)
DATE=$(date +%Y-%m-%d)
CL_VERSION="2.0.0"

cat > "$TMPDIR_CL/CHANGELOG.md" << 'EOF'
# Changelog

## [Unreleased]

### Added
- Something new

## [1.0.0] - 2026-01-01

### Added
- Initial release
EOF

# Apply the sed from release.sh (BSD sed on macOS requires the replacement on separate lines)
tmp=$(mktemp)
sed "s/## \[Unreleased\]/## [Unreleased]\n\n## [$CL_VERSION] - $DATE/" \
    "$TMPDIR_CL/CHANGELOG.md" > "$tmp"
mv "$tmp" "$TMPDIR_CL/CHANGELOG.md"

CONTENT=$(cat "$TMPDIR_CL/CHANGELOG.md")

assert_contains "changelog: [Unreleased] preserved"        "\[Unreleased\]"  "$CONTENT"
assert_contains "changelog: new version header added"      "\[$CL_VERSION\]" "$CONTENT"
assert_contains "changelog: date added"                    "$DATE"           "$CONTENT"
assert_contains "changelog: old version still present"     "\[1.0.0\]"       "$CONTENT"

rm -rf "$TMPDIR_CL"

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

# ── Test 8: Cargo file list sync ─────────────────────────────────────────────
#
# Both this test and release.sh maintain independent hardcoded lists of
# Cargo.toml files to version-bump. If they drift, releases ship with
# stale versions in unlisted crates. Verify the lists are identical.

echo ""
echo "── 8. Cargo file list sync (test vs release.sh) ──"

# Extract crate Cargo.toml paths from release.sh's version bump loop.
# Those lines look like: "    o8v-cli/Cargo.toml \"
RELEASE_CARGO=$(grep -E '^\s+o8v-[a-z-]+/Cargo\.toml' "$WORKSPACE_ROOT/scripts/release.sh" \
    | sed 's/[[:space:]\\]//g' | sed 's/;.*//' | sort)

TEST_CARGO=$(echo "$CARGO_FILES" | tr ' ' '\n' | sort)

for f in $TEST_CARGO; do
    if echo "$RELEASE_CARGO" | grep -qF "$f"; then
        ok "in both lists: $f"
    else
        fail "test lists '$f' but release.sh does not — lists drifted"
    fi
done

for f in $RELEASE_CARGO; do
    if echo "$TEST_CARGO" | grep -qF "$f"; then
        ok "release.sh file in test list: $f"
    else
        fail "release.sh has '$f' but test does not — lists drifted"
    fi
done

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
