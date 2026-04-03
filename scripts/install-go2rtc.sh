#!/usr/bin/env bash
set -euo pipefail

VERSION="${1:-1.9.7}"
INSTALL_DIR="${2:-./bin}"

# Detect platform
OS=$(uname -s | tr '[:upper:]' '[:lower:]')
ARCH=$(uname -m)

case "$ARCH" in
  x86_64|amd64) ARCH="amd64" ;;
  aarch64|arm64) ARCH="arm64" ;;
  armv7l|armhf)  ARCH="arm" ;;
  *) echo "Unsupported architecture: $ARCH"; exit 1 ;;
esac

case "$OS" in
  linux)  PLATFORM="linux" ;;
  darwin) PLATFORM="darwin" ;;
  *) echo "Unsupported OS: $OS"; exit 1 ;;
esac

BINARY="go2rtc_${PLATFORM}_${ARCH}"
URL="https://github.com/AlexxIT/go2rtc/releases/download/v${VERSION}/${BINARY}"

echo "Downloading go2rtc v${VERSION} for ${PLATFORM}/${ARCH}..."
mkdir -p "$INSTALL_DIR"
curl -fSL "$URL" -o "${INSTALL_DIR}/go2rtc"
chmod +x "${INSTALL_DIR}/go2rtc"

echo "Installed go2rtc to ${INSTALL_DIR}/go2rtc"
"${INSTALL_DIR}/go2rtc" --version 2>/dev/null || echo "(version check skipped)"
