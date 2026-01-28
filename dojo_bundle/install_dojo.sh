#!/usr/bin/env bash
set -euo pipefail

# Install MASH Dojo bundle onto a running system 🥋
# Usage: sudo ./install_dojo.sh /path/to/mash-staging

SRC="${1:-/data/mash-staging}"
if [[ ! -d "$SRC" ]]; then
  echo "Usage: $0 /path/to/mash-staging" >&2
  exit 1
fi

echo "Installing MASH Dojo system files… 🥋"

sudo mkdir -p /usr/local/lib/mash/dojo /usr/local/lib/mash/system /usr/local/bin /etc/xdg/autostart /usr/local/lib/mash/dojo/assets

# Copy dojo libs
sudo rsync -a "$SRC/usr_local_lib_mash/dojo/" /usr/local/lib/mash/dojo/
sudo rsync -a "$SRC/usr_local_lib_mash/system/" /usr/local/lib/mash/system/

# Copy launcher
sudo install -m 0755 "$SRC/usr_local_bin/mash-dojo-launch" /usr/local/bin/mash-dojo-launch

# Autostart (KDE)
sudo install -m 0644 "$SRC/autostart/mash-dojo.desktop" /etc/xdg/autostart/mash-dojo.desktop

# Assets
if [[ -f "$SRC/assets/starship.toml" ]]; then
  sudo install -m 0644 "$SRC/assets/starship.toml" /usr/local/lib/mash/dojo/assets/starship.toml
fi

# Early SSH + internet wait units (optional)
if [[ -f "$SRC/systemd/mash-early-ssh.service" ]]; then
  sudo install -m 0644 "$SRC/systemd/mash-early-ssh.service" /etc/systemd/system/mash-early-ssh.service
  sudo install -m 0755 "$SRC/systemd/early-ssh.sh" /usr/local/lib/mash/system/early-ssh.sh
  sudo systemctl enable mash-early-ssh.service || true
fi

if [[ -f "$SRC/systemd/mash-internet-wait.service" ]]; then
  sudo install -m 0644 "$SRC/systemd/mash-internet-wait.service" /etc/systemd/system/mash-internet-wait.service
  sudo install -m 0755 "$SRC/systemd/internet-wait.sh" /usr/local/lib/mash/system/internet-wait.sh
  sudo systemctl enable mash-internet-wait.service || true
fi

sudo systemctl daemon-reload || true

echo "✅ Dojo installed."
echo "➡️  Log out + back in (or reboot) and the Dojo should appear automatically."
echo "Manual launch:  /usr/local/bin/mash-dojo-launch"
