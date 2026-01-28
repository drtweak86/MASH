#!/usr/bin/env bash
set -euo pipefail

USER_NAME="${1:-drtweak}"
LOG_DIR="/data/mash-logs"
LOG_FILE="${LOG_DIR}/brave.log"
mkdir -p "$LOG_DIR"

log(){ echo "[$(date -Is)] $*" | tee -a "$LOG_FILE" ; }

log "=== Brave install + default browser ==="

# Basic connectivity check (don't hard-fail firstboot if offline)
if ! curl -fsSL --max-time 8 https://www.google.com >/dev/null 2>&1; then
  log "No internet detected; skipping Brave install for now."
  exit 0
fi

log "Installing Brave repo..."
sudo rpm --import https://brave-browser-rpm-release.s3.brave.com/brave-core.asc 2>&1 | tee -a "$LOG_FILE" || true
curl -fsSL https://brave-browser-rpm-release.s3.brave.com/brave-browser.repo | sudo tee /etc/yum.repos.d/brave-browser.repo >/dev/null

log "dnf install brave-browser"
sudo dnf install -y brave-browser 2>&1 | tee -a "$LOG_FILE" || {
  log "Brave install failed (repo/arch?)."
  exit 0
}

# Set default browser for the user (best-effort)
log "Setting default browser for user: $USER_NAME"
if id "$USER_NAME" >/dev/null 2>&1; then
  sudo -u "$USER_NAME" xdg-settings set default-web-browser brave-browser.desktop 2>&1 | tee -a "$LOG_FILE" || true
  # KDE also reads mimeapps.list; ensure http/https handlers are Brave.
  user_home="$(getent passwd "$USER_NAME" | cut -d: -f6)"
  if [[ -n "$user_home" ]]; then
    mkdir -p "$user_home/.config"
    mimefile="$user_home/.config/mimeapps.list"
    touch "$mimefile"
    grep -q '^\[Default Applications\]' "$mimefile" || echo '[Default Applications]' >> "$mimefile"
    # append if missing
    for mime in x-scheme-handler/http x-scheme-handler/https; do
      grep -q "^${mime}=" "$mimefile" || echo "${mime}=brave-browser.desktop" >> "$mimefile"
    done
    chown "$USER_NAME:$USER_NAME" "$mimefile" || true
  fi
else
  log "User $USER_NAME not found yet; default browser will be set later via Dojo."
fi

log "Done."
