#!/usr/bin/env bash
set -euo pipefail
banner() { echo "=============================================================================="; echo "$1"; echo "=============================================================================="; }
banner "Storage: ensure LABEL=DATA mounted at /data"

sudo mkdir -p /data

if ! grep -qE '^[^#]+\s+/data\s+' /etc/fstab; then
  echo "Adding fstab entry for /data"
  echo 'LABEL=DATA  /data  ext4  defaults,noatime  0  2' | sudo tee -a /etc/fstab >/dev/null
else
  echo "fstab already has /data entry."
fi

sudo mount -a || true
sudo chown "$(id -un):$(id -gn)" /data 2>/dev/null || true

echo "Done. df -h /data:"
df -h /data || true
