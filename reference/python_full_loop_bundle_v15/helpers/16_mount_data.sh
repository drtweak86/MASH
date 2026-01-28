#!/usr/bin/env bash
set -euo pipefail
USER="${1:-DrTweak}"
echo "[*] Ensuring DATA partition mounted at /data"
mkdir -p /data
if ! grep -qE '^[^#].*\s/data\s' /etc/fstab; then
  echo "LABEL=DATA  /data  ext4  defaults,noatime  0  2" >> /etc/fstab
fi
mount -a || true
chown "$USER:$USER" /data || true
