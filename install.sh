#!/usr/bin/env bash
set -euo pipefail

# ==========================
# 🥋 MASH Installer bootstrap
# ==========================
REPO_DEFAULT="drtweak86/MASH"
REPO="${REPO:-$REPO_DEFAULT}"     # override via env if needed
VERSION="${VERSION:-latest}"      # "latest" or e.g. "v0.2.2"
INSTALL_DIR="${INSTALL_DIR:-/usr/local/bin}"
BIN_NAME="mash-installer"

need() { command -v "$1" >/dev/null 2>&1 || { echo "❌ missing: $1"; exit 1; }; }

need curl
need tar
need uname
need install
need sha256sum

ARCH="$(uname -m)"
OS="$(uname -s | tr '[:upper:]' '[:lower:]')"

if [[ "$OS" != "linux" ]]; then
  echo "❌ Unsupported OS: $OS (expected linux)"
  exit 1
fi

case "$ARCH" in
  aarch64|arm64) ARCH_TAG="aarch64" ;;
  *)
    echo "❌ Unsupported arch: $ARCH (expected aarch64/arm64)"
    exit 1
    ;;
esac

if [[ "$VERSION" == "latest" ]]; then
  TGZ="mash-installer-latest-linux-${ARCH_TAG}.tar.gz"
  SHA="${TGZ}.sha256"
  URL_TGZ="https://github.com/${REPO}/releases/latest/download/${TGZ}"
  URL_SHA="https://github.com/${REPO}/releases/latest/download/${SHA}"
else
  TGZ="mash-installer-${VERSION}-linux-${ARCH_TAG}.tar.gz"
  SHA="${TGZ}.sha256"
  URL_TGZ="https://github.com/${REPO}/releases/download/${VERSION}/${TGZ}"
  URL_SHA="https://github.com/${REPO}/releases/download/${VERSION}/${SHA}"
fi

echo "🥋 MASH Installer bootstrap"
echo "   REPO        = ${REPO}"
echo "   VERSION     = ${VERSION}"
echo "   ASSET       = ${TGZ}"
echo "   URL         = ${URL_TGZ}"
echo "   INSTALL_DIR = ${INSTALL_DIR}"
echo

TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT

echo "⬇️  Downloading tarball..."
curl -fL --retry 3 --retry-delay 1 -o "$TMP/${TGZ}" "$URL_TGZ"

echo "🔐 Trying to download checksum (optional)..."
if curl -fL --retry 3 --retry-delay 1 -o "$TMP/${SHA}" "$URL_SHA"; then
  echo "✅ Checksum downloaded. Verifying..."
  (cd "$TMP" && sha256sum -c "$SHA")
else
  echo "⚠️  No checksum found (or download failed). Continuing anyway."
fi

echo "📦 Extracting..."
tar -xzf "$TMP/${TGZ}" -C "$TMP"

if [[ ! -x "$TMP/${BIN_NAME}" ]]; then
  echo "❌ Expected ${BIN_NAME} inside tarball, but not found."
  echo "   Contents:"
  ls -la "$TMP"
  exit 1
fi

echo "🧪 Quick smoke test..."
"$TMP/${BIN_NAME}" --help >/dev/null

echo "🚀 Installing to ${INSTALL_DIR}/${BIN_NAME} (sudo if needed)..."
if [[ "$(id -u)" -ne 0 ]]; then
  sudo install -m 0755 "$TMP/${BIN_NAME}" "${INSTALL_DIR}/${BIN_NAME}"
else
  install -m 0755 "$TMP/${BIN_NAME}" "${INSTALL_DIR}/${BIN_NAME}"
fi

echo
echo "✅ Installed: ${INSTALL_DIR}/${BIN_NAME}"
echo
echo "Next:"
echo "  mash-installer preflight"
echo "  mash-installer flash --help"
echo
echo "🦀🥋"
