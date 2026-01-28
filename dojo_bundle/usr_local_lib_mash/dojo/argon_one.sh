#!/usr/bin/env bash
set -euo pipefail
banner() { echo "=============================================================================="; echo "$1"; echo "=============================================================================="; }
banner "Argon One V2: install (Fedora) + enable I2C"

# 1) Ensure I2C enabled in firmware config (UEFI uses config.txt on the ESP)
ESP="/boot/efi"
if mountpoint -q "$ESP"; then
  sudo cp -an "$ESP/config.txt" "$ESP/config.txt.bak.$(date +%s)" 2>/dev/null || true
  if ! grep -q '^dtparam=i2c_arm=on' "$ESP/config.txt" 2>/dev/null; then
    echo "Enabling I2C in $ESP/config.txt"
    printf "\n# MASH: Argon One V2\n[all]\ndtparam=i2c_arm=on\n" | sudo tee -a "$ESP/config.txt" >/dev/null
  else
    echo "I2C already enabled in $ESP/config.txt"
  fi
else
  echo "⚠️  /boot/efi not mounted. Mount it then rerun this step."
fi

# 2) Install build deps + clone + install daemon
sudo dnf install -y --setopt=install_weak_deps=True gcc make git i2c-tools libi2c-devel 2>/dev/null || true

WORK="/usr/local/src"
sudo mkdir -p "$WORK"
cd "$WORK"
if [[ ! -d argononed ]]; then
  sudo git clone https://gitlab.com/DarkElvenAngel/argononed.git
else
  sudo git -C argononed pull || true
fi

cd "$WORK/argononed"
# Many installers are interactive; attempt best-effort noninteractive
if [[ -x ./argononed.sh ]]; then
  echo "Running argononed.sh (may prompt once)."
  sudo bash ./argononed.sh || true
elif [[ -f Makefile ]]; then
  sudo make || true
  sudo make install || true
else
  echo "argononed repo layout unexpected; open $WORK/argononed and install manually."
fi

echo
echo "If the fan doesn't kick in after install: reboot once (I2C overlay change)."
