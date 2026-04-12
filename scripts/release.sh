#!/bin/bash
set -euo pipefail

# Release script for 8v
# Usage: ./scripts/release.sh 0.1.0
#        ./scripts/release.sh 0.1.0 --dry-run

VERSION="${1:-}"
DRY_RUN="${2:-}"

# Colors
GREEN='\033[0;32m'
RED='\033[0;31m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Helper functions
success() {
    echo -e "${GREEN}✓${NC} $1"
}

error() {
    echo -e "${RED}✗${NC} $1" >&2
}

warn() {
    echo -e "${YELLOW}!${NC} $1"
}

step() {
    echo ""
    echo "▶ $1"
}

# Validate arguments
if [ -z "$VERSION" ]; then
    error "Version argument required"
    echo "Usage: ./scripts/release.sh 0.1.0 [--dry-run]"
    exit 1
fi

if [ -n "$DRY_RUN" ] && [ "$DRY_RUN" != "--dry-run" ]; then
    error "Invalid flag: $DRY_RUN (use --dry-run)"
    exit 1
fi

# Validate version format — must be X.Y.Z (no v-prefix, no prerelease suffix).
if ! echo "$VERSION" | grep -qE '^[0-9]+\.[0-9]+\.[0-9]+$'; then
    error "Invalid version format: '$VERSION'"
    echo "Expected: X.Y.Z (e.g. 1.2.3) — no v-prefix, no prerelease suffix"
    exit 1
fi

# Get workspace root
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WORKSPACE_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
cd "$WORKSPACE_ROOT"

step "Release v$VERSION"
if [ "$DRY_RUN" = "--dry-run" ]; then
    echo "(dry-run mode — no commits, tags, or uploads)"
fi

# ============================================================================
# 1. VERIFY PREREQUISITES
# ============================================================================

step "Checking prerequisites..."

# Clean git
if ! git diff --quiet || ! git diff --cached --quiet; then
    error "Git working tree must be clean"
    git status
    exit 1
fi
success "Git working tree is clean"

# Required tools
for cmd in cargo-zigbuild wrangler codesign xcrun; do
    if ! command -v "$cmd" >/dev/null 2>&1; then
        # sha256sum may not exist on macOS, that's ok (use shasum)
        if [ "$cmd" = "sha256sum" ]; then
            if ! command -v shasum >/dev/null 2>&1; then
                error "MISSING: shasum or sha256sum required"
                exit 1
            fi
        else
            error "MISSING: $cmd"
            if [ "$cmd" = "cargo-zigbuild" ]; then
                echo "  Install: cargo install cargo-zigbuild"
            elif [ "$cmd" = "wrangler" ]; then
                echo "  Install: npm install -g wrangler"
            elif [ "$cmd" = "codesign" ] || [ "$cmd" = "xcrun" ]; then
                echo "  Install: Xcode command line tools"
            elif [ "$cmd" = "zig" ]; then
                echo "  Install: brew install zig"
            fi
            exit 1
        fi
    fi
done
success "All required tools found"

# zig is available (used by cargo-zigbuild)
if ! command -v zig >/dev/null 2>&1; then
    error "zig must be installed (brew install zig)"
    exit 1
fi
success "zig available"

# Apple notarization credentials (API key auth)
APPLE_SECRETS="$HOME/.8v/secrets/apple"
NOTARIZE_ENV="$APPLE_SECRETS/notarize.env"
CODESIGN_ENV="$APPLE_SECRETS/codesign.env"

if [ -f "$NOTARIZE_ENV" ]; then
    . "$NOTARIZE_ENV"
else
    error "MISSING: $NOTARIZE_ENV"
    echo "  See docs/design/release.md for setup"
    exit 1
fi

if [ -z "${APPLE_API_KEY:-}" ] || [ ! -f "${APPLE_API_KEY:-}" ]; then
    error "MISSING: Apple API key (.p8 file) at $APPLE_API_KEY"
    exit 1
fi
if [ -z "${APPLE_KEY_ID:-}" ]; then
    error "MISSING: APPLE_KEY_ID in $NOTARIZE_ENV"
    exit 1
fi
if [ -z "${APPLE_ISSUER_ID:-}" ]; then
    error "MISSING: APPLE_ISSUER_ID in $NOTARIZE_ENV"
    exit 1
fi
success "Apple notarization credentials found (API key: $APPLE_KEY_ID)"

# Developer ID certificate in keychain
if ! security find-identity -v -p codesigning | grep -q "Developer ID Application"; then
    error "MISSING: Developer ID Application certificate in keychain"
    exit 1
fi
success "Developer ID certificate found in keychain"

# Wrangler authenticated
if ! wrangler r2 bucket list >/dev/null 2>&1; then
    error "MISSING: wrangler not authenticated"
    echo "  Run: wrangler login"
    exit 1
fi
success "wrangler is authenticated"

# ============================================================================
# 2. RUN CHECKS
# ============================================================================

step "Running checks..."

# Build first so we can use local binary for checks
cargo build -p o8v 2>/dev/null
LOCAL_8V="$WORKSPACE_ROOT/target/debug/8v"

if ! "$LOCAL_8V" check . > /dev/null; then
    error "8v check failed"
    exit 1
fi
success "8v check passed"

if ! "$LOCAL_8V" fmt . --check > /dev/null; then
    error "8v fmt --check failed"
    exit 1
fi
success "8v fmt --check passed"

if ! cargo test --workspace > /dev/null 2>&1; then
    error "cargo test failed"
    cargo test --workspace
    exit 1
fi
success "cargo test passed"

# ============================================================================
# 3. BUILD ALL PLATFORMS
# ============================================================================

step "Building all platform binaries..."

mkdir -p dist
rm -f dist/8v-*

# darwin-arm64 (native)
echo "  → darwin-arm64 (native)..."
cargo build --release -p o8v 2>&1 | grep -E "(Compiling|Finished)" || true
cp target/release/8v dist/8v-darwin-arm64
success "darwin-arm64 built"

# darwin-x64 (native cross-compile)
echo "  → darwin-x64 (cross-compile)..."
rustup target add x86_64-apple-darwin > /dev/null 2>&1 || true
cargo build --release -p o8v --target x86_64-apple-darwin 2>&1 | grep -E "(Compiling|Finished)" || true
cp target/x86_64-apple-darwin/release/8v dist/8v-darwin-x64
success "darwin-x64 built"

# linux-x64 (zigbuild, no Docker)
echo "  → linux-x64 (zigbuild)..."
cargo zigbuild --release -p o8v --target x86_64-unknown-linux-musl 2>&1 | grep -E "(Compiling|Finished)" || true
cp target/x86_64-unknown-linux-musl/release/8v dist/8v-linux-x64
success "linux-x64 built"

# linux-arm64 (zigbuild, no Docker)
echo "  → linux-arm64 (zigbuild)..."
cargo zigbuild --release -p o8v --target aarch64-unknown-linux-musl 2>&1 | grep -E "(Compiling|Finished)" || true
cp target/aarch64-unknown-linux-musl/release/8v dist/8v-linux-arm64
success "linux-arm64 built"

# ============================================================================
# 4. SIGN MACOS BINARIES
# ============================================================================

step "Signing macOS binaries..."

# Auto-detect signing identity
IDENTITY=$(security find-identity -v -p codesigning \
    | grep "Developer ID Application" | head -1 \
    | awk -F'"' '{print $2}')

if [ -z "$IDENTITY" ]; then
    error "Could not find Developer ID Application certificate"
    exit 1
fi

echo "  Using identity: $IDENTITY"

for bin in dist/8v-darwin-arm64 dist/8v-darwin-x64; do
    codesign --sign "$IDENTITY" --options runtime --timestamp "$bin" 2>&1 | head -1 || true
done
success "macOS binaries signed"

# ============================================================================
# 5. VERIFY SIGNATURES
# ============================================================================

step "Verifying signatures..."

for bin in dist/8v-darwin-arm64 dist/8v-darwin-x64; do
    if ! codesign --verify --verbose "$bin" > /dev/null 2>&1; then
        error "Signature verification failed for $(basename "$bin")"
        exit 1
    fi
done
success "Signatures verified"

# ============================================================================
# 6. NOTARIZE MACOS BINARIES
# ============================================================================

step "Notarizing macOS binaries (this may take 1-5 minutes)..."

for bin in dist/8v-darwin-arm64 dist/8v-darwin-x64; do
    echo "  → $(basename "$bin")..."
    ZIP_FILE="${bin}.zip"

    # Create zip for notarization
    cd dist
    zip -q "$(basename "$ZIP_FILE")" "$(basename "$bin")"
    cd "$WORKSPACE_ROOT"

    # Submit for notarization (API key auth)
    if ! xcrun notarytool submit "$ZIP_FILE" \
        --key "$APPLE_API_KEY" \
        --key-id "$APPLE_KEY_ID" \
        --issuer "$APPLE_ISSUER_ID" \
        --wait 2>&1; then
        error "Notarization rejected for $(basename "$bin")"
        echo "  Check: xcrun notarytool log <submission-id> ..."
        rm -f "$ZIP_FILE"
        exit 1
    fi

    rm -f "$ZIP_FILE"
done
success "Notarization complete"

# ============================================================================
# 7. VERIFY BINARY SIZES + GENERATE CHECKSUMS
# ============================================================================

step "Verifying binary sizes..."

MAX_SIZE=$((20 * 1024 * 1024))  # 20 MB in bytes

for f in dist/8v-*; do
    SIZE=$(stat -f%z "$f" 2>/dev/null || stat -c%s "$f" 2>/dev/null || echo 0)
    SIZE_MB=$((SIZE / 1024 / 1024))
    SIZE_KB=$((SIZE / 1024))

    if [ "$SIZE" -gt "$MAX_SIZE" ]; then
        warn "$(basename "$f") is over 20MB (${SIZE_MB}MB)"
    fi
    echo "  $(basename "$f"): ${SIZE_KB}KB"
done

step "Generating checksums..."

cd dist
if command -v sha256sum >/dev/null 2>&1; then
    sha256sum 8v-* > checksums.txt
else
    shasum -a 256 8v-* > checksums.txt
fi
cat checksums.txt
cd "$WORKSPACE_ROOT"
success "Checksums generated"

# ============================================================================
# 8. DRY-RUN: EXIT HERE
# ============================================================================

if [ "$DRY_RUN" = "--dry-run" ]; then
    step "DRY-RUN COMPLETE"
    echo ""
    echo "Summary:"
    echo "  Version: v$VERSION"
    echo "  Binaries:"
    ls -lh dist/8v-* | awk '{print "    " $9 " (" $5 ")"}'
    echo "  Checksums: dist/checksums.txt"
    echo ""
    echo "Next steps (when ready for real release):"
    echo "  ./scripts/release.sh $VERSION"
    echo ""
    exit 0
fi

# ============================================================================
# 9. BUMP VERSION IN CARGO.TOML FILES
# ============================================================================

step "Bumping version to $VERSION..."

# Update all crate Cargo.toml files (workspace root has no version field — skip it)
for cargo_file in \
    o8v/Cargo.toml \
    o8v-core/Cargo.toml \
    o8v-fs/Cargo.toml \
    o8v-process/Cargo.toml \
    o8v-project/Cargo.toml \
    o8v-testkit/Cargo.toml \
    o8v-workspace/Cargo.toml; do

    if [ -f "$cargo_file" ]; then
        sed -i '' "s/^version = \".*\"/version = \"$VERSION\"/" "$cargo_file"
    fi
done

# Regenerate Cargo.lock
cargo check -p o8v > /dev/null 2>&1
success "Version bumped to $VERSION"

# ============================================================================
# 10. UPDATE CHANGELOG
# ============================================================================

step "Updating CHANGELOG.md..."

DATE=$(date +%Y-%m-%d)
sed -i '' "s/## \[Unreleased\]/## [Unreleased]\n\n## [$VERSION] - $DATE/" CHANGELOG.md
success "CHANGELOG.md updated"

# ============================================================================
# 11. COMMIT + TAG
# ============================================================================

step "Creating release commit and tag..."

git add -A
git commit -m "Release v$VERSION"
git tag -a "v$VERSION" -m "Release v$VERSION"
success "Commit and tag created"

# ============================================================================
# 12. UPLOAD TO R2
# ============================================================================

step "Uploading binaries to R2..."

BUCKET="8v-releases"

for file in dist/8v-darwin-arm64 dist/8v-darwin-x64 dist/8v-linux-arm64 dist/8v-linux-x64 dist/checksums.txt; do
    echo "  → $(basename "$file")..."
    if ! wrangler r2 object put --remote "${BUCKET}/v${VERSION}/$(basename "$file")" \
        --file "$file" > /dev/null 2>&1; then
        error "Failed to upload $(basename "$file")"
        exit 1
    fi
done
success "All binaries uploaded"

step "Verifying uploads on R2..."

for file in 8v-darwin-arm64 8v-darwin-x64 8v-linux-arm64 8v-linux-x64 checksums.txt; do
    HTTP_CODE=$(curl -s -o /dev/null -w "%{http_code}" \
        "https://releases.8vast.io/v${VERSION}/${file}")

    if [ "$HTTP_CODE" != "200" ]; then
        error "$file not accessible (HTTP $HTTP_CODE)"
        exit 1
    fi
    echo "  ✓ $file (HTTP $HTTP_CODE)"
done
success "All binaries verified on R2"

# ============================================================================
# 13. PUBLISH VERSION.TXT (POINT OF NO RETURN)
# ============================================================================

step "Publishing version.txt..."

VERSION_TMP="/tmp/8v-version.txt"
echo "$VERSION" > "$VERSION_TMP"

wrangler r2 object put --remote "8v-releases/latest/version.txt" \
    --file "$VERSION_TMP" > /dev/null 2>&1
rm -f "$VERSION_TMP"

# Verify
REMOTE=$(curl -s https://releases.8vast.io/latest/version.txt)
if [ "$REMOTE" != "$VERSION" ]; then
    error "version.txt shows '$REMOTE', expected '$VERSION'"
    exit 1
fi
success "Published v$VERSION"

# ============================================================================
# 14. PUSH TO GIT
# ============================================================================

step "Pushing to git..."

CURRENT_BRANCH=$(git rev-parse --abbrev-ref HEAD)
git push origin "$CURRENT_BRANCH"
git push origin "v$VERSION"
success "Pushed to origin"

# ============================================================================
# DONE
# ============================================================================

step "✓ Release v$VERSION complete!"
echo ""
echo "Release details:"
echo "  Version: v$VERSION"
echo "  Tag: $(git describe --tags)"
echo "  Binaries: https://releases.8vast.io/v${VERSION}/"
echo "  Latest: https://releases.8vast.io/latest/version.txt"
echo ""
