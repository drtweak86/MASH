#!/usr/bin/env bash
set -euo pipefail

# MASH Dojo entrypoint 🥋
BASE="/usr/local/lib/mash/dojo"
LOG_DIR="${XDG_STATE_HOME:-$HOME/.local/state}/mash"
mkdir -p "$LOG_DIR"
LOG_FILE="$LOG_DIR/dojo.log"

# If user has completed Dojo, bail quietly.
COMPLETED_FLAG="${XDG_STATE_HOME:-$HOME/.local/state}/mash/dojo.completed"
if [[ -f "$COMPLETED_FLAG" ]]; then
  exit 0
fi

# Source helpers + menu
source "$BASE/graphics.sh"
source "$BASE/menu.sh"

# Run menu (logs tee'd)
menu_main 2>&1 | tee -a "$LOG_FILE"
