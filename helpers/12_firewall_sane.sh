#!/usr/bin/env bash
set -euo pipefail

# Usage:
#   12_firewall_sane.sh /mnt/ninja_dst/root_sub_root /mnt/mash_data_stage/bootstrap
#
# Installs a LAN-safe firewalld setup for SSH + MOSH + mDNS.
# Logs at runtime: /data/mash-logs/early-ssh.log (handled by early-ssh.sh)

ROOT="${1:?need target root path}"
STAGE="${2:?need staging dir path}"

mkdir -p "${ROOT}/etc/systemd/system" "${ROOT}/usr/local/lib/mash/system" "${ROOT}/etc/systemd/system/multi-user.target.wants"

install -m 0644 "${STAGE}/systemd/mash-early-ssh.service" "${ROOT}/etc/systemd/system/mash-early-ssh.service"
install -m 0755 "${STAGE}/systemd/early-ssh.sh" "${ROOT}/usr/local/lib/mash/system/early-ssh.sh"

ln -sf ../mash-early-ssh.service "${ROOT}/etc/systemd/system/multi-user.target.wants/mash-early-ssh.service"
