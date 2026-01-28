#!/usr/bin/env bash
set -euo pipefail
banner() { echo "=============================================================================="; echo "$1"; echo "=============================================================================="; }
banner "Borg: install + quick-start"

sudo dnf install -y borgbackup 2>/dev/null || true

cat <<'TXT'

Next steps (example):
  mkdir -p /data/borgrepo
  borg init --encryption=repokey-blake2 /data/borgrepo

Backup example:
  borg create --stats /data/borgrepo::"{hostname}-{now}" /home

When you're ready, we can bake your exact policy (excludes, compression, timers).

TXT
