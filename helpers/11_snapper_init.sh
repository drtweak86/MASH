#!/usr/bin/env bash
set -euo pipefail
USER="${1:-DrTweak}"
echo "[*] Snapper init for / (btrfs)"
dnf install -y snapper || true
snapper -c root create-config / || true
chmod a+rx /.snapshots || true
chown ":$USER" /.snapshots || true
