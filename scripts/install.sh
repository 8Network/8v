#!/bin/sh
# 8v install script — safe, simple, portable
# Usage: curl -fsSL https://install.8vast.io | sh

set -e

# Cleanup on exit (temp files, kill downloads on error)
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
# Validate base URL
# ============================================================================

validate_base_url() {
  BASE_URL="$1"

  # Allow https://, http://localhost, http://127.0.0.1
  case "$BASE_URL" in
    https://*)
      echo "$BASE_URL"
      return 0
      ;;
    http://localhost*)
      echo "$BASE_URL"
      return 0
      ;;
    http://127.0.0.1*)
      echo "$BASE_URL"
      return 0
      ;;
    *)
      echo "Error: _8V_BASE_URL must be https:// (or http://localhost for testing)" >&2
      exit 1
      ;;
  esac
}

# ============================================================================
# Get latest version
# ============================================================================

get_version() {
  BASE_URL="$1"
  VERSION_URL="${BASE_URL}/latest/version.txt"

  VERSION=$(curl -fsSL --connect-timeout 15 --max-time 120 "$VERSION_URL" 2>/dev/null | tr -d '\n\r')

  if [ -z "$VERSION" ]; then
    echo "Error: failed to fetch latest version from $VERSION_URL" >&2
    exit 1
  fi

  echo "$VERSION"
}

# ============================================================================
# Download binary and checksums
# ============================================================================

download_binary() {
  BINARY_NAME="8v-$1"
  VERSION="$2"
  BASE_URL="$3"
  BINARY_URL="${BASE_URL}/v${VERSION}/${BINARY_NAME}"
  CHECKSUMS_URL="${BASE_URL}/v${VERSION}/checksums.txt"

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

  # Extract the expected checksum for this binary from checksums.txt
  # Format: "hash  filename"
  EXPECTED=$(grep "$BINARY_NAME\$" "$CHECKSUM_FILE" | awk '{print $1}')

  if [ -z "$EXPECTED" ]; then
    echo "Error: checksum not found in checksums.txt for $BINARY_NAME" >&2
    exit 1
  fi

  echo "Verifying checksum..."

  # Compute actual checksum (use shasum on macOS, sha256sum on Linux)
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
  # Try each directory in PATH
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

  # Fallback: ~/.local/bin
  LOCAL_BIN="$HOME/.local/bin"
  mkdir -p "$LOCAL_BIN"

  # Check if fallback is in PATH
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

  # Fallback is not in PATH — tell user how to fix it
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

  # Check if we got the fallback without PATH inclusion (set by find_install_dir)
  if [ "$INSTALL_DIR_IN_PATH" != "true" ]; then
    SHELL_NAME=$(basename "$SHELL")
    case "$SHELL_NAME" in
      zsh)
        RC_FILE="$HOME/.zshrc"
        ;;
      bash)
        RC_FILE="$HOME/.bashrc"
        ;;
      *)
        RC_FILE="$HOME/.profile"
        ;;
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

  # Verify the installed binary runs
  if ! "$INSTALL_DIR/8v" --version >/dev/null 2>&1; then
    echo "Error: installed binary failed verification (--version)" >&2
    exit 1
  fi
}

# ============================================================================
# Main
# ============================================================================

TEMP_DIR=$(mktemp -d)

# Validate and store BASE_URL from environment or use default
VALIDATED_BASE_URL=$(validate_base_url "${_8V_BASE_URL:-https://releases.8vast.io}")

PLATFORM=$(detect_platform)
VERSION=$(get_version "$VALIDATED_BASE_URL")

download_binary "$PLATFORM" "$VERSION" "$VALIDATED_BASE_URL"
verify_checksum "$PLATFORM"

INSTALL_DIR=$(find_install_dir)
install_binary "$INSTALL_DIR"

echo ""
echo "Success! 8v v$VERSION installed to $INSTALL_DIR/8v"
echo ""
echo "Try it: 8v help"
