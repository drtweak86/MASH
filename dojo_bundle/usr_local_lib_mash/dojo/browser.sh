#!/usr/bin/env bash
set -euo pipefail

USER_NAME="${1:-$SUDO_USER}"
USER_NAME="${USER_NAME:-drtweak}"

LOG_DIR="/data/mash-logs"
LOG_FILE="${LOG_DIR}/dojo-browser.log"
mkdir -p "$LOG_DIR"
log(){ echo "[$(date -Is)] $*" | tee -a "$LOG_FILE" ; }

echo
echo "🌐 Browser dojo: Brave install + set default (user: ${USER_NAME})"
echo

if ! command -v curl >/dev/null 2>&1; then
  sudo dnf install -y curl
fi

if ! curl -fsSL --max-time 8 https://www.google.com >/dev/null 2>&1; then
  log "No internet detected; cannot install Brave yet."
  echo "❌ No internet yet. Come back after Wi‑Fi is up."
  exit 0
fi

log "Importing Brave key + repo..."
sudo rpm --import https://brave-browser-rpm-release.s3.brave.com/brave-core.asc || true
curl -fsSL https://brave-browser-rpm-release.s3.brave.com/brave-browser.repo | sudo tee /etc/yum.repos.d/brave-browser.repo >/dev/null

log "Installing brave-browser..."
sudo dnf install -y brave-browser || { echo "❌ Brave install failed."; exit 0; }

log "Setting default browser..."
if id "$USER_NAME" >/dev/null 2>&1; then
  sudo -u "$USER_NAME" xdg-settings set default-web-browser brave-browser.desktop || true
  echo "✅ Default browser set to Brave for ${USER_NAME} (best-effort)."
else
  echo "⚠️ User ${USER_NAME} not found; log in once then rerun this option."
fi

echo
echo "Done. Launch Brave from the app menu."
