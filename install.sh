#!/usr/bin/env bash
set -euo pipefail

REPO="Drtweak/MASH"
BIN_DIR="${HOME}/MASH/bin"
DOJO_DIR="${HOME}/MASH/dojo"
ARCH="$(uname -m)"

mkdir -p "$BIN_DIR" "$DOJO_DIR"

case "$ARCH" in
  aarch64) ASSET_BIN="mash-installer-aarch64" ;;
  x86_64)  ASSET_BIN="mash-installer-x86_64" ;;
  *)
    echo "❌ Unsupported arch: $ARCH"
    exit 1
    ;;
esac

echo "🦀 Installing MASH Installer from $REPO for $ARCH"
echo "📦 бинарь: $ASSET_BIN"

LATEST_JSON="$(curl -fsSL "https://api.github.com/repos/${REPO}/releases/latest")"
DL_BIN_URL="$(echo "$LATEST_JSON" | grep -Eo '"browser_download_url":[ ]*"[^"]+' | cut -d'"' -f4 | grep "/${ASSET_BIN}$" | head -n1)"
DL_DOJO_URL="$(echo "$LATEST_JSON" | grep -Eo '"browser_download_url":[ ]*"[^"]+' | cut -d'"' -f4 | grep "/dojo_bundle.zip$" | head -n1)"

if [[ -z "${DL_BIN_URL:-}" ]]; then
  echo "❌ Could not find ${ASSET_BIN} in latest release assets."
  exit 1
fi

curl -fL "$DL_BIN_URL" -o "${BIN_DIR}/mash-installer"
chmod +x "${BIN_DIR}/mash-installer"

if [[ -n "${DL_DOJO_URL:-}" ]]; then
  echo "🥋 Downloading Dojo bundle"
  curl -fL "$DL_DOJO_URL" -o "${DOJO_DIR}/dojo_bundle.zip"
  (cd "$DOJO_DIR" && unzip -o dojo_bundle.zip >/dev/null)
else
  echo "⚠️ No dojo_bundle.zip in latest release (ok for Phase 1A/1B)."
fi

echo "✅ Installed: ${BIN_DIR}/mash-installer"
echo
echo "▶️ Running: mash-installer preflight --dry-run"
"${BIN_DIR}/mash-installer" preflight --dry-run
