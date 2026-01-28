#!/usr/bin/env bash
set -euo pipefail
echo "[*] Packages: desktop/media bits (safe subset)"
dnf install -y --skip-unavailable --setopt=install_weak_deps=True \
  pipewire pipewire-pulseaudio alsa-utils pavucontrol \
  gstreamer1-plugins-ugly gstreamer1-plugins-bad-free-extras || true
