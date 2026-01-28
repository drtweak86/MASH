#!/usr/bin/env bash
set -euo pipefail
USER="${1:-DrTweak}"
echo "[*] Zsh + Starship (fallback installer)"
dnf install -y --skip-unavailable zsh || true
if ! command -v starship >/dev/null 2>&1; then
  curl -fsSL https://starship.rs/install.sh | sh -s -- -y || true
fi
HOME_DIR="$(getent passwd "$USER" | cut -d: -f6)"
ZSHRC="$HOME_DIR/.zshrc"
mkdir -p "$HOME_DIR"
touch "$ZSHRC"
grep -q 'starship init zsh' "$ZSHRC" || echo 'eval "$(starship init zsh)"' >> "$ZSHRC"
chsh -s /usr/bin/zsh "$USER" || true
