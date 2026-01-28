#!/usr/bin/env bash
set -euo pipefail

# Usage:
#   03_stage_starship_toml.sh /mnt/mash_data_stage/bootstrap /path/to/starship.toml
#
# Copies starship.toml into staging assets so Dojo can install it to /etc/starship.toml.

STAGE="${1:?need staging dir path}"
SRC="${2:?need starship.toml path}"

mkdir -p "${STAGE}/assets"
cp -av "${SRC}" "${STAGE}/assets/starship.toml"
