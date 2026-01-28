#!/usr/bin/env bash
set -euo pipefail

fix_audio() {
  echo "== Audio sanity 🔊 =="
  sudo dnf -y install alsa-utils pipewire wireplumber || true
  sudo systemctl --user enable --now pipewire wireplumber 2>/dev/null || true

  # Raspberry Pi HDMI / onboard audio: ensure snd_bcm2835 enabled (already via config.txt)
  # Try to restart pipewire stack
  systemctl --user restart pipewire wireplumber 2>/dev/null || true

  echo "Devices:"
  aplay -l || true
  wpctl status || true
  echo "✅ Audio fix attempt complete."
}

case "${1:-}" in
  --fix) shift; fix_audio "$@";;
  ""|--help|-h) : ;;
  *) echo "Unknown arg: $1" >&2; exit 2;;
esac
