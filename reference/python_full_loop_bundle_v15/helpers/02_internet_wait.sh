#!/usr/bin/env bash
set -euo pipefail

# Usage:
#   02_internet_wait.sh /mnt/ninja_dst/root_sub_root /mnt/mash_data_stage/bootstrap
#
# Installs a oneshot unit that blocks the Dojo/bootstrap until network is usable.
# Logs: /data/mash-logs/internet-wait.log

ROOT="${1:?need target root path}"
STAGE="${2:?need staging dir path}"

mkdir -p "${ROOT}/etc/systemd/system" "${ROOT}/usr/local/lib/mash/system" "${ROOT}/etc/systemd/system/multi-user.target.wants"

install -m 0644 "${STAGE}/systemd/mash-internet-wait.service" "${ROOT}/etc/systemd/system/mash-internet-wait.service"
install -m 0755 "${STAGE}/systemd/internet-wait.sh" "${ROOT}/usr/local/lib/mash/system/internet-wait.sh"

ln -sf ../mash-internet-wait.service "${ROOT}/etc/systemd/system/multi-user.target.wants/mash-internet-wait.service"
