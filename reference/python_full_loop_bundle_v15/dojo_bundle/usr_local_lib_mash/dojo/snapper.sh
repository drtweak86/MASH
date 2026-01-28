#!/usr/bin/env bash
set -euo pipefail
banner() { echo "=============================================================================="; echo "$1"; echo "=============================================================================="; }
banner "Snapper: Atomic Shield for / (Btrfs)"

sudo dnf install -y snapper btrfs-assistant 2>/dev/null || true

# Create config if missing (snapper returns non-zero if already covered)
sudo snapper -c root create-config / || true

# Allow traversal
sudo chmod a+rx /.snapshots || true

echo
echo "Snapper ready. GUI: btrfs-assistant"
echo "CLI: snapper -c root list"
