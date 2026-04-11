#!/bin/sh
# Full install E2E test.
#
# Simulates the exact user journey:
#   curl -fsSL https://install.8vast.io | sh
#   8v init --yes
#   8v check
#
# Instead of hitting the real release server, we:
#   1. Build the binary locally (cargo build)
#   2. Serve it from a temp HTTP server (python3)
#   3. Run scripts/install.sh with _8V_BASE_URL pointing to localhost
#   4. Verify the installed binary works: --version, init --yes, check
#
# Usage:
#   sh scripts/test-install.sh
#
# Requirements: cargo, python3, shasum or sha256sum, sh

set -eu

WORKSPACE_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$WORKSPACE_ROOT"

# ── Helpers ───────────────────────────────────────────────────────────────────

fail() {
  echo "FAIL: $*" >&2
  exit 1
}

pass() {
  echo "ok: $*"
}

# ── Cleanup ───────────────────────────────────────────────────────────────────

SERVER_PID=""
SERVE_DIR=""
INSTALL_DIR=""
PROJECT_DIR=""

cleanup() {
  [ -n "$SERVER_PID" ] && kill "$SERVER_PID" 2>/dev/null || true
  [ -n "$SERVE_DIR" ]  && rm -rf "$SERVE_DIR"
  [ -n "$INSTALL_DIR" ] && rm -rf "$INSTALL_DIR"
  [ -n "$PROJECT_DIR" ] && rm -rf "$PROJECT_DIR"
}
trap cleanup EXIT

# ── Step 1: Build ─────────────────────────────────────────────────────────────

echo "Building 8v binary..."
cargo build -p o8v --quiet
BINARY="$WORKSPACE_ROOT/target/debug/8v"
[ -x "$BINARY" ] || fail "binary not found at $BINARY"
pass "binary built: $BINARY"

# ── Step 2: Detect platform (same logic as install.sh) ────────────────────────

OS=$(uname -s)
ARCH=$(uname -m)
case "$OS" in
  Darwin)
    case "$ARCH" in
      arm64)   PLATFORM="darwin-arm64" ;;
      x86_64)  PLATFORM="darwin-x64"   ;;
      *) fail "unsupported macOS arch: $ARCH" ;;
    esac ;;
  Linux)
    case "$ARCH" in
      x86_64)  PLATFORM="linux-x64"   ;;
      aarch64) PLATFORM="linux-arm64" ;;
      *) fail "unsupported Linux arch: $ARCH" ;;
    esac ;;
  *) fail "unsupported OS: $OS" ;;
esac
pass "platform: $PLATFORM"

# ── Step 3: Stage fake release tree ──────────────────────────────────────────
#
# install.sh expects:
#   $BASE_URL/latest/version.txt
#   $BASE_URL/v{VERSION}/8v-{PLATFORM}
#   $BASE_URL/v{VERSION}/checksums.txt

VERSION="test-$(date +%s)"
BINARY_NAME="8v-$PLATFORM"

SERVE_DIR=$(mktemp -d)
mkdir -p "$SERVE_DIR/latest" "$SERVE_DIR/v${VERSION}"

echo "$VERSION" > "$SERVE_DIR/latest/version.txt"
cp "$BINARY" "$SERVE_DIR/v${VERSION}/${BINARY_NAME}"

# Generate checksum (shasum on macOS, sha256sum on Linux)
if command -v shasum >/dev/null 2>&1; then
  CHECKSUM=$(shasum -a 256 "$SERVE_DIR/v${VERSION}/${BINARY_NAME}" | awk '{print $1}')
elif command -v sha256sum >/dev/null 2>&1; then
  CHECKSUM=$(sha256sum "$SERVE_DIR/v${VERSION}/${BINARY_NAME}" | awk '{print $1}')
else
  fail "neither shasum nor sha256sum found"
fi

echo "$CHECKSUM  ${BINARY_NAME}" > "$SERVE_DIR/v${VERSION}/checksums.txt"
pass "release tree staged at $SERVE_DIR (version=$VERSION, checksum=${CHECKSUM:0:16}...)"

# ── Step 4: Start local HTTP server ──────────────────────────────────────────

find_free_port() {
  python3 -c "import socket; s=socket.socket(); s.bind(('', 0)); print(s.getsockname()[1]); s.close()"
}
PORT=$(find_free_port)
python3 -m http.server "$PORT" --directory "$SERVE_DIR" >/dev/null 2>&1 &
SERVER_PID=$!

# Wait until server accepts connections
RETRIES=10
until curl -sf "http://localhost:$PORT/latest/version.txt" >/dev/null 2>&1; do
  RETRIES=$((RETRIES - 1))
  [ "$RETRIES" -eq 0 ] && fail "HTTP server did not start on port $PORT"
  sleep 0.2
done
pass "HTTP server running on port $PORT (pid=$SERVER_PID)"

# ── Step 5: Run install.sh against local server ───────────────────────────────

INSTALL_DIR=$(mktemp -d)

# Prepend our install dir to PATH so install.sh places the binary there.
# install.sh picks the first writable PATH directory.
export PATH="$INSTALL_DIR:$PATH"

echo "Running install.sh..."
_8V_BASE_URL="http://localhost:$PORT" sh "$WORKSPACE_ROOT/scripts/install.sh"

# ── Step 6: Verify binary is installed ───────────────────────────────────────

INSTALLED="$INSTALL_DIR/8v"
[ -x "$INSTALLED" ] || fail "binary not found at $INSTALLED after install"
pass "binary installed at $INSTALLED"

# ── Step 7: Smoke test — version ─────────────────────────────────────────────

VERSION_OUT=$("$INSTALLED" --version 2>&1)
echo "$VERSION_OUT" | grep -q "8v" || fail "--version output does not contain '8v': $VERSION_OUT"
pass "8v --version: $VERSION_OUT"

# ── Step 8: 8v init --yes ────────────────────────────────────────────────────

PROJECT_DIR=$(mktemp -d)
"$INSTALLED" init --yes "$PROJECT_DIR" || fail "8v init --yes failed"

[ -d "$PROJECT_DIR/.8v" ] || fail ".8v/ not created by init --yes"
pass "8v init --yes created .8v/"

# ── Step 9: 8v check ─────────────────────────────────────────────────────────

# Empty directory exits with code 2 (nothing to check).
# This is the expected outcome for an empty project.
set +e
"$INSTALLED" check "$PROJECT_DIR"
EXIT_CODE=$?
set -e

[ "$EXIT_CODE" -eq 2 ] || fail "8v check on empty dir must exit 2, got $EXIT_CODE"
pass "8v check exited 2 (nothing to check — expected for empty project)"

# ── Step 10: Verify series.json was written ───────────────────────────────────

SERIES="$PROJECT_DIR/.8v/series.json"
[ -f "$SERIES" ] || fail "series.json not written after check"

# Basic structure check — must be valid JSON with run_id and timestamp
python3 << PYEOF || fail "series.json invalid"
import json, sys
with open('$SERIES') as f:
    d = json.load(f)
assert d.get('run_id'), 'run_id missing or empty'
assert d.get('timestamp', 0) > 0, 'timestamp missing or zero'
PYEOF
pass "series.json written and valid"

# ── Step 11: Verify _8V_BASE_URL validation rejects plain http ───────────────
#
# install.sh must refuse to run when _8V_BASE_URL is a non-localhost http://
# URL. This prevents a misconfiguration from silently downloading over plain
# http in production.

set +e
BAD_URL_OUT=$(_8V_BASE_URL="http://evil.example.com" sh "$WORKSPACE_ROOT/scripts/install.sh" 2>&1)
BAD_URL_EXIT=$?
set -e

[ "$BAD_URL_EXIT" -ne 0 ] || fail "_8V_BASE_URL=http://evil.example.com must exit non-zero"
echo "$BAD_URL_OUT" | grep -qi "error" || fail "bad-URL rejection must print an error message"
pass "_8V_BASE_URL=http://evil.example.com correctly rejected (exit $BAD_URL_EXIT)"

# ── Step 12: Tampered checksum causes install to fail ────────────────────────
#
# Even if the binary downloads successfully, a wrong hash in checksums.txt
# must cause install.sh to abort. Verifies the checksum is actually enforced,
# not just fetched and ignored.

TAMPER_SERVE_DIR=$(mktemp -d)
mkdir -p "$TAMPER_SERVE_DIR/latest" "$TAMPER_SERVE_DIR/v${VERSION}"
echo "$VERSION" > "$TAMPER_SERVE_DIR/latest/version.txt"
cp "$SERVE_DIR/v${VERSION}/${BINARY_NAME}" "$TAMPER_SERVE_DIR/v${VERSION}/${BINARY_NAME}"
# All-zeros hash — wrong, but looks like a valid SHA256
printf '0000000000000000000000000000000000000000000000000000000000000000  %s\n' \
    "${BINARY_NAME}" > "$TAMPER_SERVE_DIR/v${VERSION}/checksums.txt"

TAMPER_PORT=$(find_free_port)
python3 -m http.server "$TAMPER_PORT" --directory "$TAMPER_SERVE_DIR" >/dev/null 2>&1 &
TAMPER_PID=$!

RETRIES=10
until curl -sf "http://localhost:$TAMPER_PORT/latest/version.txt" >/dev/null 2>&1; do
  RETRIES=$((RETRIES - 1))
  if [ "$RETRIES" -eq 0 ]; then
    kill "$TAMPER_PID" 2>/dev/null || true
    rm -rf "$TAMPER_SERVE_DIR"
    fail "tamper HTTP server did not start on port $TAMPER_PORT"
  fi
  sleep 0.2
done

TAMPER_INSTALL_DIR=$(mktemp -d)
PATH_SAVE="$PATH"
export PATH="$TAMPER_INSTALL_DIR:$PATH"

set +e
TAMPER_OUT=$(_8V_BASE_URL="http://localhost:$TAMPER_PORT" sh "$WORKSPACE_ROOT/scripts/install.sh" 2>&1)
TAMPER_EXIT=$?
set -e

export PATH="$PATH_SAVE"
kill "$TAMPER_PID" 2>/dev/null || true
rm -rf "$TAMPER_SERVE_DIR" "$TAMPER_INSTALL_DIR"

[ "$TAMPER_EXIT" -ne 0 ] || fail "tampered checksum must cause install to fail"
echo "$TAMPER_OUT" | grep -qi "mismatch\|checksum" || fail "must print checksum error, got: $TAMPER_OUT"
pass "tampered checksum correctly detected and install aborted"

# ── Done ─────────────────────────────────────────────────────────────────────

echo ""
echo "All install E2E checks passed."
echo "  install.sh  →  $INSTALLED"
echo "  8v --version"
echo "  8v init --yes"
echo "  8v check (exit 2)"
echo "  .8v/series.json written"
echo "  _8V_BASE_URL http:// correctly rejected"
echo "  tampered checksum correctly rejected"
