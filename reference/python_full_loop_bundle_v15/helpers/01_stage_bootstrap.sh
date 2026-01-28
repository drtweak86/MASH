#!/usr/bin/env bash
set -euo pipefail
DATA_MOUNT="${1:-/mnt/data}"
SRC_ROOT="${2:?need path to mash_helpers root}"
DST="$DATA_MOUNT/bootstrap"
echo "[*] Staging bootstrap into $DST"
mkdir -p "$DST"
rsync -a --delete "$SRC_ROOT/" "$DST/"
chmod +x "$DST/mash_forge.py" "$DST/helpers/"*.sh || true
sync
echo "[+] Staged. On Fedora first boot run: sudo /data/bootstrap/mash_forge.py firstboot ..."
