\
#!/usr/bin/env bash
set -euo pipefail

USER_NAME="${1:-drtweak}"
LOG_DIR="/data/mash-logs"
LOG_FILE="${LOG_DIR}/brave.log"

mkdir -p "$LOG_DIR"
exec > >(tee -a "$LOG_FILE") 2>&1

echo "================================================================================"
echo "🌐 Brave browser + default browser setup"
echo "================================================================================"

# Ensure we have tooling
dnf install -y --setopt=install_weak_deps=True dnf-plugins-core curl ca-certificates || true

# Add Brave repo (idempotent)
REPO_FILE="/etc/yum.repos.d/brave-browser.repo"
if [[ ! -f "$REPO_FILE" ]]; then
  echo "➕ Adding Brave repo: $REPO_FILE"
  rpm --import https://brave-browser-rpm-release.s3.brave.com/brave-core.asc || true
  cat >"$REPO_FILE" <<'EOF'
[brave-browser]
name=Brave Browser
baseurl=https://brave-browser-rpm-release.s3.brave.com/x86_64/
enabled=1
gpgcheck=1
gpgkey=https://brave-browser-rpm-release.s3.brave.com/brave-core.asc
EOF
  # NOTE: Pi is aarch64; Brave may not exist for aarch64 in this repo.
fi

echo "📦 Installing brave-browser (best-effort; may be unavailable on aarch64)"
dnf install -y --skip-unavailable brave-browser || true

# If not installed, leave a hint and bail gracefully
if ! rpm -q brave-browser >/dev/null 2>&1; then
  echo "⚠️ brave-browser not installed (likely no aarch64 build in repo)."
  echo "   Dojo will offer alternatives (Firefox) and you can revisit later."
  dnf install -y --setopt=install_weak_deps=True firefox || true
if rpm -q firefox >/dev/null 2>&1; then
  echo "🦊 Falling back to Firefox as default browser."
  if id "$USER_NAME" >/dev/null 2>&1; then
    sudo -u "$USER_NAME" xdg-settings set default-web-browser firefox.desktop || true
    sudo -u "$USER_NAME" xdg-mime default firefox.desktop x-scheme-handler/http || true
    sudo -u "$USER_NAME" xdg-mime default firefox.desktop x-scheme-handler/https || true
    sudo -u "$USER_NAME" xdg-mime default firefox.desktop text/html || true
  fi
fi
exit 0
fi

# Set default browser for the target user (best-effort)
echo "🔧 Setting default browser to Brave for user: $USER_NAME"
if id "$USER_NAME" >/dev/null 2>&1; then
  sudo -u "$USER_NAME" xdg-settings set default-web-browser brave-browser.desktop || true
  sudo -u "$USER_NAME" xdg-mime default brave-browser.desktop x-scheme-handler/http || true
  sudo -u "$USER_NAME" xdg-mime default brave-browser.desktop x-scheme-handler/https || true
  sudo -u "$USER_NAME" xdg-mime default brave-browser.desktop text/html || true

  # Also patch ~/.config/mimeapps.list (KDE respects this)
  HOME_DIR="$(getent passwd "$USER_NAME" | cut -d: -f6)"
  MIMEAPPS="${HOME_DIR}/.config/mimeapps.list"
  sudo -u "$USER_NAME" mkdir -p "${HOME_DIR}/.config" || true
  if [[ -f "$MIMEAPPS" ]]; then
    sudo -u "$USER_NAME" grep -q '^\[Default Applications\]' "$MIMEAPPS" || echo "[Default Applications]" | sudo -u "$USER_NAME" tee -a "$MIMEAPPS" >/dev/null
  else
    echo "[Default Applications]" | sudo -u "$USER_NAME" tee "$MIMEAPPS" >/dev/null
  fi
  {
    echo "x-scheme-handler/http=brave-browser.desktop"
    echo "x-scheme-handler/https=brave-browser.desktop"
    echo "text/html=brave-browser.desktop"
  } | sudo -u "$USER_NAME" tee -a "$MIMEAPPS" >/dev/null
else
  echo "⚠️ user '$USER_NAME' not found yet; skipping default-browser binding."
fi

echo "✅ Brave step complete."
