#!/usr/bin/env bash
set -euo pipefail

# MASH Dojo menu 🥋 (ncurses/dialog)
# This runs in the user's session. It should NOT assume root.
BASE="/usr/local/lib/mash/dojo"
STATE_DIR="${XDG_STATE_HOME:-$HOME/.local/state}/mash"
mkdir -p "$STATE_DIR"
COMPLETED_FLAG="$STATE_DIR/dojo.completed"

have() { command -v "$1" >/dev/null 2>&1; }

_need_dialog() {
  if ! have dialog; then
    echo "⚠️ 'dialog' not installed. Install it with: sudo dnf -y install dialog"
    return 1
  fi
  return 0
}

mark_done() {
  touch "$COMPLETED_FLAG"
}

action_disable_dpms() {
  sudo "$BASE/graphics.sh" --apply-dpms-off || true
}

action_preview_starship() {
  "$BASE/bootstrap.sh" --preview-starship || true
}

action_firewall() {
  sudo "$BASE/firewall.sh" --sane-lan || true
}

action_audio() {
  sudo "$BASE/audio.sh" --fix || true
}

action_bootstrap() {
  sudo "$BASE/bootstrap.sh" --run || true
}

menu_main() {
  _need_dialog || return 0

  local choice
  while true; do
    choice=$(dialog --clear --no-shadow --title "MASH Dojo 🥋" \
      --menu "Choose your move:" 18 72 10 \
      1 "🔊 Fix audio (PipeWire/ALSA sanity)" \
      2 "🖥️  Disable DPMS + screensaver (no blackouts)" \
      3 "🛡️  Firewall sane (LAN SSH/Mosh allowed)" \
      4 "⭐ Preview Starship theme" \
      5 "🔥 Run MASH bootstrap (packages, extras)" \
      9 "✅ Exit & don't show again" \
      3>&1 1>&2 2>&3) || true

    case "${choice:-}" in
      1) action_audio ;;
      2) action_disable_dpms ;;
      3) action_firewall ;;
      4) action_preview_starship ;;
      5) action_bootstrap ;;
      9) mark_done; clear; echo "✅ Dojo completed. See you in the dojo, captain. 🥋"; return 0 ;;
      *) clear; return 0 ;;
    esac
  done
}
