#!/usr/bin/env bash
set -euo pipefail

# Firewalld helpers 🛡️

sane_lan() {
  echo "== Firewalld sane LAN rules 🛡️ =="
  if ! systemctl list-unit-files | grep -q '^firewalld\.service'; then
    echo "firewalld not present. Installing..."
    sudo dnf -y install firewalld || true
  fi
  sudo systemctl enable --now firewalld || true

  local zone=""
  if sudo firewall-cmd --get-zones | tr ' ' '\n' | grep -qx home; then zone="home"; fi
  if [[ -z "$zone" ]] && sudo firewall-cmd --get-zones | tr ' ' '\n' | grep -qx trusted; then zone="trusted"; fi
  if [[ -z "$zone" ]]; then zone="$(sudo firewall-cmd --get-default-zone)"; fi

  sudo firewall-cmd --permanent --zone="$zone" --add-service=ssh || true
  sudo firewall-cmd --permanent --zone="$zone" --add-port=60000-61000/udp || true  # mosh
  sudo firewall-cmd --reload || true

  echo "✅ Allowed: ssh + mosh in zone '$zone'"
}

case "${1:-}" in
  --sane-lan) shift; sane_lan "$@";;
  ""|--help|-h) : ;;
  *) echo "Unknown arg: $1" >&2; exit 2;;
esac
