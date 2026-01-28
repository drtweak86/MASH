#!/usr/bin/env bash
set -euo pipefail
USER="${1:-DrTweak}"
echo "[*] KDE screensaver/DPMS nuke (best effort)"
sudo -u "$USER" sh -c 'kwriteconfig5 --file kscreenlockerrc --group Daemon --key Autolock false' || true
sudo -u "$USER" sh -c 'kwriteconfig5 --file powerdevilrc --group AC --group SuspendSession --key suspendType 0' || true
sudo -u "$USER" sh -c 'xset s off' || true
sudo -u "$USER" sh -c 'xset -dpms' || true
