#!/usr/bin/env bash
set -euo pipefail

# Graphics helpers + DPMS/screen blanking controls 🖥️

banner() {
  local msg="$1"
  printf "\n========================================\n%s\n========================================\n" "$msg"
}

apply_dpms_off() {
  banner "Disable DPMS + screensaver 🛑😴"

  # KDE settings (best-effort)
  if command -v kwriteconfig5 >/dev/null 2>&1; then
    kwriteconfig5 --file kscreenlockerrc --group Daemon --key Autolock false || true
    kwriteconfig5 --file powermanagementprofilesrc --group AC --group DPMSControl --key idleTime 0 || true
    kwriteconfig5 --file powermanagementprofilesrc --group AC --group DimDisplay --key idleTime 0 || true
    kwriteconfig5 --file powermanagementprofilesrc --group AC --group SuspendSession --key idleTime 0 || true
  fi

  # X11 immediate
  if command -v xset >/dev/null 2>&1 && [[ -n "${DISPLAY:-}" ]]; then
    xset s off || true
    xset -dpms || true
  fi

  # Wayland (KDE) best-effort: poke screen saver off
  if command -v qdbus >/dev/null 2>&1; then
    qdbus org.freedesktop.ScreenSaver /ScreenSaver SetActive false >/dev/null 2>&1 || true
  fi

  echo "✅ DPMS/screensaver tweaks applied (best-effort)."
  echo "Tip: may require logout/login for some KDE power settings."
}

case "${1:-}" in
  --apply-dpms-off) shift; apply_dpms_off "$@";;
  ""|--help|-h) : ;;
  *) echo "Unknown arg: $1" >&2; exit 2;;
esac
