#!/usr/bin/env bash
# Release script for 8v.
# Usage: ./release.sh <version>        # full release
#        ./release.sh <version> --dry-run  # build + sign only
#
# Must be run from the inner repo root (products/vast/oss/8v/).

set -euo pipefail
cd "$(dirname "$0")"

VERSION="${1:-}"
DRY_RUN=false
if [ "${2:-}" = "--dry-run" ]; then
    DRY_RUN=true
fi

if [ -z "$VERSION" ]; then
    echo "Usage: $0 <version> [--dry-run]"
    echo "Example: $0 0.2.0"
    exit 1
fi

# Validate version is strict semver (no pre-release, no build metadata)
if ! echo "$VERSION" | grep -qE '^[0-9]+\.[0-9]+\.[0-9]+$'; then
    echo "ERROR: Version must be X.Y.Z (got '$VERSION')"
    exit 1
fi

echo "=== 8v Release v$VERSION ==="
if [ "$DRY_RUN" = true ]; then
    echo "(dry-run mode — will build and sign but not publish)"
fi

# ─────────────────────────────────────────────────────────────
# 1. Verify prerequisites
# ─────────────────────────────────────────────────────────────
echo ""
echo "--- Step 1: Prerequisites ---"

FAIL=0

# Clean working tree
if [ -n "$(git status --porcelain)" ]; then
    echo "FAIL: working tree is not clean"
    git status --short
    FAIL=1
fi

# Required tools
for tool in cargo-zigbuild wrangler codesign xcrun zig; do
    command -v "$tool" >/dev/null || { echo "FAIL: $tool not found"; FAIL=1; }
done

# Required env vars for notarization
for var in APPLE_ID APPLE_TEAM_ID APPLE_APP_SPECIFIC_PASSWORD; do
    [ -n "${!var}" ] || { echo "FAIL: $var not set"; FAIL=1; }
done

# Developer ID certificate in keychain
security find-identity -v -p codesigning | grep -q "Developer ID Application" \
    || { echo "FAIL: Developer ID certificate not in keychain"; FAIL=1; }

# wrangler authenticated
wrangler r2 bucket list >/dev/null 2>&1 \
    || { echo "FAIL: wrangler not authenticated (run wrangler login)"; FAIL=1; }

# Workspace versioning configured
grep -q '^version' Cargo.toml \
    || { echo "FAIL: [workspace.package] version missing in Cargo.toml"; FAIL=1; }
grep -q 'version.workspace = true' o8v/Cargo.toml \
    || { echo "FAIL: version.workspace = true missing in o8v/Cargo.toml"; FAIL=1; }

[ "$FAIL" -eq 0 ] || { echo "Prerequisites failed. Aborting."; exit 1; }
echo "All prerequisites met."

# ─────────────────────────────────────────────────────────────
# 2. Run checks
# ─────────────────────────────────────────────────────────────
echo ""
echo "--- Step 2: Checks ---"

8v check .
8v fmt . --check
cargo test --workspace

echo "All checks passed."

# ─────────────────────────────────────────────────────────────
# 3. Bump version + update changelog
# ─────────────────────────────────────────────────────────────
echo ""
echo "--- Step 3: Version bump ---"

BEFORE=$(grep -c '^version' Cargo.toml)
sed -i '' "s/^version = \".*\"/version = \"$VERSION\"/" Cargo.toml
AFTER=$(grep -c '^version' Cargo.toml)
if [ "$BEFORE" -ne "$AFTER" ]; then
    echo "ERROR: sed changed the number of version lines ($BEFORE → $AFTER)"
    exit 1
fi

# Regenerate Cargo.lock with new version
cargo check -p o8v

# Update CHANGELOG.md
DATE=$(date +%Y-%m-%d)
sed -i '' "s/## \[Unreleased\]/## [Unreleased]\n\n## [$VERSION] - $DATE/" CHANGELOG.md

echo "Version bumped to $VERSION"

# ─────────────────────────────────────────────────────────────
# 4. Build all platforms
# ─────────────────────────────────────────────────────────────
echo ""
echo "--- Step 4: Build ---"

rm -rf dist
mkdir -p dist

# Native: darwin-arm64
echo "Building darwin-arm64 (native)..."
cargo build --release -p o8v
cp target/release/8v dist/8v-darwin-arm64

# Cross: darwin-x64
echo "Building darwin-x64 (native cross)..."
rustup target add x86_64-apple-darwin 2>/dev/null || true
cargo build --release -p o8v --target x86_64-apple-darwin
cp target/x86_64-apple-darwin/release/8v dist/8v-darwin-x64

# Cross: linux-x64
echo "Building linux-x64 (zigbuild)..."
cargo zigbuild --release -p o8v --target x86_64-unknown-linux-musl
cp target/x86_64-unknown-linux-musl/release/8v dist/8v-linux-x64

# Cross: linux-arm64
echo "Building linux-arm64 (zigbuild)..."
cargo zigbuild --release -p o8v --target aarch64-unknown-linux-musl
cp target/aarch64-unknown-linux-musl/release/8v dist/8v-linux-arm64

# Cross: windows-x64
echo "Building windows-x64 (zigbuild)..."
cargo zigbuild --release -p o8v --target x86_64-pc-windows-gnu
cp target/x86_64-pc-windows-gnu/release/8v.exe dist/8v-windows-x64.exe

# Cross: windows-arm64
echo "Building windows-arm64 (zigbuild)..."
cargo zigbuild --release -p o8v --target aarch64-pc-windows-gnu
cp target/aarch64-pc-windows-gnu/release/8v.exe dist/8v-windows-arm64.exe

# Verify Linux binaries are statically linked
for bin in dist/8v-linux-x64 dist/8v-linux-arm64; do
    if ! file "$bin" | grep -q "statically linked"; then
        echo "ERROR: $bin is not statically linked"
        file "$bin"
        exit 1
    fi
done

echo "All 6 binaries built."

# ─────────────────────────────────────────────────────────────
# 5. Sign macOS binaries
# ─────────────────────────────────────────────────────────────
echo ""
echo "--- Step 5: Code signing ---"

IDENTITY=$(security find-identity -v -p codesigning \
    | grep "Developer ID Application" | head -1 \
    | awk -F'"' '{print $2}')

codesign --sign "$IDENTITY" --options runtime --timestamp dist/8v-darwin-arm64
codesign --sign "$IDENTITY" --options runtime --timestamp dist/8v-darwin-x64

# Verify
codesign --verify --verbose dist/8v-darwin-arm64
codesign --verify --verbose dist/8v-darwin-x64

echo "macOS binaries signed and verified."

# ─────────────────────────────────────────────────────────────
# 6. Notarize macOS binaries
# ─────────────────────────────────────────────────────────────
echo ""
echo "--- Step 6: Notarization ---"
echo "This may take 1-5 minutes per binary."

for bin in dist/8v-darwin-arm64 dist/8v-darwin-x64; do
    zip "${bin}.zip" "$bin"
    # macOS has no `timeout` (GNU coreutils). Use perl wrapper.
    perl -e 'alarm 600; exec @ARGV' xcrun notarytool submit "${bin}.zip" \
        --apple-id "$APPLE_ID" \
        --team-id "$APPLE_TEAM_ID" \
        --password "$APPLE_APP_SPECIFIC_PASSWORD" \
        --wait
    STATUS=$?
    rm "${bin}.zip"
    if [ $STATUS -ne 0 ]; then
        echo "ERROR: Notarization rejected for $(basename $bin)"
        echo "Check: xcrun notarytool log <submission-id> ..."
        exit 1
    fi
done

echo "macOS binaries notarized."

# ─────────────────────────────────────────────────────────────
# 7. Verify binary sizes + generate checksums
# ─────────────────────────────────────────────────────────────
echo ""
echo "--- Step 7: Checksums ---"

echo "Binary sizes:"
ls -lh dist/8v-*
for f in dist/8v-*; do
    SIZE=$(stat -f%z "$f" 2>/dev/null || stat -c%s "$f")
    if [ "$SIZE" -gt 20971520 ]; then
        echo "WARNING: $(basename $f) is over 20MB ($SIZE bytes)"
    fi
done

# Verify dist/ contains only expected files
EXPECTED=6
ACTUAL=$(ls -1 dist/8v-* | wc -l | tr -d ' ')
if [ "$ACTUAL" -ne "$EXPECTED" ]; then
    echo "ERROR: expected $EXPECTED binaries in dist/, found $ACTUAL"
    ls -la dist/
    exit 1
fi

# Generate checksums
cd dist
if command -v sha256sum >/dev/null; then
    sha256sum 8v-* > checksums.txt
else
    shasum -a 256 8v-* > checksums.txt
fi
cat checksums.txt
cd ..

echo "Checksums generated."

# ─────────────────────────────────────────────────────────────
# 8. Dry-run stops here
# ─────────────────────────────────────────────────────────────
if [ "$DRY_RUN" = true ]; then
    echo ""
    echo "=== Dry run complete ==="
    echo "Built and signed v$VERSION for all platforms."
    echo "Reverting version bump..."
    git checkout -- Cargo.toml Cargo.lock CHANGELOG.md
    echo "Done. Run without --dry-run to publish."
    exit 0
fi

# ─────────────────────────────────────────────────────────────
# 9. Commit + annotated tag
# ─────────────────────────────────────────────────────────────
echo ""
echo "--- Step 9: Commit + tag ---"

git add -A
git commit -m "Release v$VERSION"
git tag -a "v$VERSION" -m "Release v$VERSION"

echo "Committed and tagged v$VERSION"

# ─────────────────────────────────────────────────────────────
# 10. Upload to R2
# ─────────────────────────────────────────────────────────────
echo ""
echo "--- Step 10: Upload to R2 ---"

BUCKET="8v-releases"

# Upload checksums FIRST, then binaries.
wrangler r2 object put "${BUCKET}/v${VERSION}/checksums.txt" \
    --file dist/checksums.txt --remote

for file in dist/8v-darwin-arm64 dist/8v-darwin-x64 dist/8v-linux-arm64 dist/8v-linux-x64 dist/8v-windows-x64.exe dist/8v-windows-arm64.exe; do
    wrangler r2 object put "${BUCKET}/v${VERSION}/$(basename $file)" \
        --file "$file" --remote
done

# Verify EVERY binary is accessible
for file in 8v-darwin-arm64 8v-darwin-x64 8v-linux-arm64 8v-linux-x64 8v-windows-x64.exe 8v-windows-arm64.exe checksums.txt; do
    HTTP_CODE=$(curl -s -o /dev/null -w "%{http_code}" \
        "https://releases.8vast.io/v${VERSION}/${file}")
    if [ "$HTTP_CODE" != "200" ]; then
        echo "ERROR: $file not accessible (HTTP $HTTP_CODE)"
        exit 1
    fi
done
echo "All binaries verified on R2."

# ─────────────────────────────────────────────────────────────
# 11. Publish (point of no return)
# ─────────────────────────────────────────────────────────────
echo ""
echo "--- Step 11: Publish version.txt (POINT OF NO RETURN) ---"

echo "$VERSION" > /tmp/8v-version.txt
wrangler r2 object put "8v-releases/latest/version.txt" \
    --file /tmp/8v-version.txt --remote
rm /tmp/8v-version.txt

# Cache-busting query param — Cloudflare ignores client Cache-Control headers
REMOTE=$(curl -s "https://releases.8vast.io/latest/version.txt?t=$(date +%s)")
if [ "$REMOTE" != "$VERSION" ]; then
    echo "ERROR: version.txt shows '$REMOTE', expected '$VERSION'"
    exit 1
fi
echo "Published v$VERSION"

# ─────────────────────────────────────────────────────────────
# 12. Push
# ─────────────────────────────────────────────────────────────
echo ""
echo "--- Step 12: Push ---"

git push origin main
git push origin "v$VERSION"

echo "Pushed to github.com/8network/8v"

# ─────────────────────────────────────────────────────────────
# 13. Create GitHub Release
# ─────────────────────────────────────────────────────────────
echo ""
echo "--- Step 13: GitHub Release ---"

sed -n "/^## \[$VERSION\]/,/^## \[/p" CHANGELOG.md | sed '$d' > /tmp/8v-release-notes.md

if [ ! -s /tmp/8v-release-notes.md ]; then
    echo "ERROR: No changelog entry found for version $VERSION"
    echo "Expected '## [$VERSION]' header in CHANGELOG.md"
    exit 1
fi

cat >> /tmp/8v-release-notes.md << 'EOF'

---
## Install
```
8v upgrade
```
EOF

gh release create "v$VERSION" \
    --repo 8network/8v \
    --title "v$VERSION" \
    --notes-file /tmp/8v-release-notes.md

rm /tmp/8v-release-notes.md

echo "GitHub Release created."

# ─────────────────────────────────────────────────────────────
# 14. Verify from user perspective
# ─────────────────────────────────────────────────────────────
echo ""
echo "--- Step 14: Verify ---"
echo "Release v$VERSION complete."
echo ""
echo "Manual verification:"
echo "  1. Run '8v upgrade' from an older binary"
echo "  2. Check https://github.com/8network/8v/releases/tag/v$VERSION"
echo "  3. Verify checksums: curl https://releases.8vast.io/v$VERSION/checksums.txt"
