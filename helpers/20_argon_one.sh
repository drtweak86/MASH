#!/usr/bin/env bash
set -euo pipefail
USER="${1:-DrTweak}"
echo "[*] Argon One V2 (best effort)"
dnf install -y --skip-unavailable git gcc make dtc i2c-tools libi2c-devel || true
install -d /opt/argononed
if [ ! -d /opt/argononed/.git ]; then
  rm -rf /opt/argononed
  git clone https://gitlab.com/DarkElvenAngel/argononed.git /opt/argononed || true
fi
if [ -x /opt/argononed/install.sh ]; then
  bash -lc "cd /opt/argononed && ./install.sh" || true
else
  echo "[!] Repo layout differs; review /opt/argononed contents."
fi
echo "[+] If fan still dead: ensure dtparam=i2c_arm=on in /boot/efi/config.txt and reboot."
