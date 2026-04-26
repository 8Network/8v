#!/bin/sh
# 8v install script — safe, simple, portable
# Usage: curl -fsSL https://install.8vast.io | sh

set -e

cleanup() {
  rm -rf "$TEMP_DIR"
}
trap cleanup EXIT

# ============================================================================
# Detect platform
# ============================================================================

detect_platform() {
  OS=$(uname -s)
  ARCH=$(uname -m)

  case "$OS" in
    Darwin)
      case "$ARCH" in
        arm64) echo "darwin-arm64" ;;
        x86_64) echo "darwin-x64" ;;
        *)
          echo "Error: unsupported macOS architecture: $ARCH" >&2
          echo "Expected: arm64 (Apple Silicon) or x86_64 (Intel)" >&2
          exit 1
          ;;
      esac
      ;;
    Linux)
      case "$ARCH" in
        x86_64) echo "linux-x64" ;;
        aarch64) echo "linux-arm64" ;;
        *)
          echo "Error: unsupported Linux architecture: $ARCH" >&2
          echo "Expected: x86_64 or aarch64" >&2
          exit 1
          ;;
      esac
      ;;
    *)
      echo "Error: unsupported operating system: $OS" >&2
      echo "Expected: Darwin (macOS) or Linux" >&2
      exit 1
      ;;
  esac
}

# ============================================================================
# Get latest version (follow GitHub /releases/latest redirect)
# ============================================================================

get_version() {
  REPO="$1"
  RELEASES_URL="https://github.com/${REPO}/releases/latest"

  # GitHub redirects /releases/latest → /releases/tag/vX.Y.Z. Parse Location.
  REDIRECT=$(curl -fsSI --connect-timeout 15 --max-time 30 "$RELEASES_URL" 2>/dev/null \
    | grep -i '^location:' | tr -d '\r' | awk '{print $2}')

  if [ -z "$REDIRECT" ]; then
    echo "Error: failed to resolve latest release from $RELEASES_URL" >&2
    exit 1
  fi

  TAG=${REDIRECT##*/}
  VERSION=${TAG#v}

  if [ -z "$VERSION" ]; then
    echo "Error: could not parse version from redirect: $REDIRECT" >&2
    exit 1
  fi

  echo "$VERSION"
}

# ============================================================================
# Download binary and checksums
# ============================================================================

download_binary() {
  PLATFORM="$1"
  VERSION="$2"
  REPO="$3"
  BINARY_NAME="8v-${PLATFORM}"
  BASE="https://github.com/${REPO}/releases/download/v${VERSION}"
  BINARY_URL="${BASE}/${BINARY_NAME}"
  CHECKSUMS_URL="${BASE}/checksums.txt"

  echo "Downloading $BINARY_NAME v$VERSION..."

  if ! curl -fsSL --connect-timeout 15 --max-time 120 "$BINARY_URL" -o "$TEMP_DIR/8v-binary"; then
    echo "Error: failed to download binary from $BINARY_URL" >&2
    exit 1
  fi

  if ! curl -fsSL --connect-timeout 15 --max-time 120 "$CHECKSUMS_URL" -o "$TEMP_DIR/checksums.txt"; then
    echo "Error: failed to download checksums from $CHECKSUMS_URL" >&2
    exit 1
  fi
}

# ============================================================================
# Verify checksum
# ============================================================================

verify_checksum() {
  BINARY_NAME="8v-$1"
  CHECKSUM_FILE="$TEMP_DIR/checksums.txt"
  BINARY_PATH="$TEMP_DIR/8v-binary"

  EXPECTED=$(grep "$BINARY_NAME\$" "$CHECKSUM_FILE" | awk '{print $1}')

  if [ -z "$EXPECTED" ]; then
    echo "Error: checksum not found in checksums.txt for $BINARY_NAME" >&2
    exit 1
  fi

  echo "Verifying checksum..."

  if command -v shasum >/dev/null 2>&1; then
    ACTUAL=$(shasum -a 256 "$BINARY_PATH" | awk '{print $1}')
  elif command -v sha256sum >/dev/null 2>&1; then
    ACTUAL=$(sha256sum "$BINARY_PATH" | awk '{print $1}')
  else
    echo "Error: neither shasum nor sha256sum found" >&2
    exit 1
  fi

  if [ "$ACTUAL" != "$EXPECTED" ]; then
    echo "Error: checksum mismatch for $BINARY_NAME" >&2
    echo "  Expected: $EXPECTED" >&2
    echo "  Got:      $ACTUAL" >&2
    exit 1
  fi

  echo "Checksum verified."
}

# ============================================================================
# Find install location
# ============================================================================

find_install_dir() {
  OLD_IFS="$IFS"
  IFS=":"
  for dir in $PATH; do
    IFS="$OLD_IFS"
    if [ -w "$dir" ] 2>/dev/null; then
      echo "$dir"
      INSTALL_DIR_IN_PATH=true
      return 0
    fi
  done
  IFS="$OLD_IFS"

  LOCAL_BIN="$HOME/.local/bin"
  mkdir -p "$LOCAL_BIN"

  OLD_IFS="$IFS"
  IFS=":"
  for dir in $PATH; do
    IFS="$OLD_IFS"
    if [ "$dir" = "$LOCAL_BIN" ]; then
      echo "$LOCAL_BIN"
      INSTALL_DIR_IN_PATH=true
      return 0
    fi
  done
  IFS="$OLD_IFS"

  echo "$LOCAL_BIN"
  INSTALL_DIR_IN_PATH=false
  return 0
}

# ============================================================================
# Install binary
# ============================================================================

install_binary() {
  INSTALL_DIR="$1"
  BINARY_PATH="$TEMP_DIR/8v-binary"

  if [ "$INSTALL_DIR_IN_PATH" != "true" ]; then
    SHELL_NAME=$(basename "$SHELL")
    case "$SHELL_NAME" in
      zsh)  RC_FILE="$HOME/.zshrc" ;;
      bash) RC_FILE="$HOME/.bashrc" ;;
      *)    RC_FILE="$HOME/.profile" ;;
    esac

    echo ""
    echo "Note: $INSTALL_DIR is not in your PATH."
    echo "To use 8v, add this line to your $RC_FILE:"
    echo ""
    echo "  export PATH=\"\$HOME/.local/bin:\$PATH\""
    echo ""
    echo "Then run: source $RC_FILE"
    echo ""
  fi

  echo "Installing to $INSTALL_DIR..."
  cp "$BINARY_PATH" "$INSTALL_DIR/8v"
  chmod +x "$INSTALL_DIR/8v"

  if ! "$INSTALL_DIR/8v" --version >/dev/null 2>&1; then
    echo "Error: installed binary failed verification (--version)" >&2
    exit 1
  fi
}

# ============================================================================
# Main
# ============================================================================

TEMP_DIR=$(mktemp -d)

REPO="${_8V_REPO:-8network/8v}"

PLATFORM=$(detect_platform)
VERSION=$(get_version "$REPO")

download_binary "$PLATFORM" "$VERSION" "$REPO"
verify_checksum "$PLATFORM"

INSTALL_DIR=$(find_install_dir)
install_binary "$INSTALL_DIR"

echo ""
echo "Success! 8v v$VERSION installed to $INSTALL_DIR/8v"
echo ""
echo "Try it: 8v help"
