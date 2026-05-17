#!/usr/bin/env bash
# Downloads a pre-built MediaMTX binary for the current platform.
#
# Usage:
#   bash install-mediamtx.sh [version] [install-dir]
#   bash install-mediamtx.sh 1.9.3 ./bin
set -euo pipefail

VERSION="${1:-1.12.2}"
INSTALL_DIR="${2:-./bin}"

# Detect platform
OS=$(uname -s | tr '[:upper:]' '[:lower:]')
ARCH=$(uname -m)

case "$ARCH" in
  x86_64|amd64) ARCH="amd64" ;;
  aarch64|arm64) ARCH="arm64v8" ;;
  armv7l|armhf)  ARCH="armv7" ;;
  *) echo "Unsupported architecture: $ARCH"; exit 1 ;;
esac

case "$OS" in
  linux)  PLATFORM="linux" ;;
  darwin) PLATFORM="darwin" ;;
  *) echo "Unsupported OS: $OS"; exit 1 ;;
esac

ARCHIVE="mediamtx_v${VERSION}_${PLATFORM}_${ARCH}.tar.gz"
URL="https://github.com/bluenviron/mediamtx/releases/download/v${VERSION}/${ARCHIVE}"

echo "Downloading MediaMTX v${VERSION} for ${PLATFORM}/${ARCH}..."
mkdir -p "$INSTALL_DIR"

TMP=$(mktemp -d)
trap 'rm -rf "$TMP"' EXIT

curl -fSL "$URL" -o "${TMP}/${ARCHIVE}"
tar -xzf "${TMP}/${ARCHIVE}" -C "$TMP" mediamtx

mv "${TMP}/mediamtx" "${INSTALL_DIR}/mediamtx"
chmod +x "${INSTALL_DIR}/mediamtx"

echo "Installed MediaMTX to ${INSTALL_DIR}/mediamtx"
"${INSTALL_DIR}/mediamtx" --version 2>/dev/null || echo "(version check skipped — mediamtx may not support --version on this build)"
