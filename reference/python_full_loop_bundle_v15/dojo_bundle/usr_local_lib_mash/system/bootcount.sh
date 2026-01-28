#!/usr/bin/env bash
set -euo pipefail

STATE_DIR="/var/lib/mash"
mkdir -p "$STATE_DIR"
COUNT_FILE="$STATE_DIR/bootcount"
LOGDIR="/data/mash-logs"
mkdir -p "$LOGDIR"
LOG="$LOGDIR/bootcount.log"

count=0
if [[ -f "$COUNT_FILE" ]]; then
  count="$(cat "$COUNT_FILE" 2>/dev/null || echo 0)"
fi
if [[ ! "$count" =~ ^[0-9]+$ ]]; then count=0; fi

count=$((count+1))
echo "$count" > "$COUNT_FILE"

echo "=== $(date -Is) bootcount=$count ===" | tee -a "$LOG"
