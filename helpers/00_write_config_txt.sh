#!/usr/bin/env bash
set -euo pipefail
EFI_MOUNT="${1:-/boot/efi}"
CFG="$EFI_MOUNT/config.txt"
echo "[*] Writing safe Pi4 UEFI config.txt -> $CFG"
cat >"$CFG" <<'EOF'
arm_64bit=1
enable_uart=1
enable_gic=1
armstub=RPI_EFI.fd
disable_commandline_tags=2

[pi4]
dtoverlay=upstream-pi4

[all]
# Add overlays here if needed
EOF
sync
