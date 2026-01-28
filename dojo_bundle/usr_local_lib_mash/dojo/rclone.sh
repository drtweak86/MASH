#!/usr/bin/env bash
set -euo pipefail
banner() { echo "=============================================================================="; echo "$1"; echo "=============================================================================="; }
banner "rclone: install + config"

sudo dnf install -y rclone 2>/dev/null || true

cat <<'TXT'

Run:
  rclone config

Then test:
  rclone lsd <remote>:

When you're ready, we can add:
  - systemd timers
  - encrypted remotes
  - bandwidth schedules

TXT
