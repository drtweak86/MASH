#!/bin/sh
set -eu

# ---- Argon One prerequisite: enable I2C in config.txt ----
: "${BINARIES_DIR:?}"

CFG1="${BINARIES_DIR}/rpi-firmware/config.txt"
CFG2="${BINARIES_DIR}/config.txt"

ensure_line() {
  file="$1"
  line="$2"
  [ -f "$file" ] || return 0
  grep -qxF "$line" "$file" || printf '\n%s\n' "$line" >> "$file"
}

ensure_line "$CFG1" "dtparam=i2c_arm=on"
ensure_line "$CFG2" "dtparam=i2c_arm=on"
: "${BINARIES_DIR:?BINARIES_DIR not set}"

EXT_DIR="/home/drtweak/frankenpi/external"
BOOT_SRC="${EXT_DIR}/bootfiles"

FW_DIR="${BINARIES_DIR}/rpi-firmware"
VMLINUX="${BINARIES_DIR}/Image"

# sanity
[ -d "${FW_DIR}" ] || { echo "ERROR: missing ${FW_DIR}"; exit 1; }
[ -f "${VMLINUX}" ] || { echo "ERROR: missing ${VMLINUX}"; exit 1; }
[ -f "${BOOT_SRC}/config.txt" ] || { echo "ERROR: missing ${BOOT_SRC}/config.txt"; exit 1; }
[ -f "${BOOT_SRC}/cmdline.txt" ] || { echo "ERROR: missing ${BOOT_SRC}/cmdline.txt"; exit 1; }

# 1) Install known-good boot config
cp -f "${BOOT_SRC}/config.txt"  "${FW_DIR}/config.txt"
cp -f "${BOOT_SRC}/cmdline.txt" "${FW_DIR}/cmdline.txt"
cp -f "${FW_DIR}/config.txt"  "${BINARIES_DIR}/config.txt"
cp -f "${FW_DIR}/cmdline.txt" "${BINARIES_DIR}/cmdline.txt"
# 2) Ensure the firmware filenames the Pi expects exist
# Some firmware drops ship as start4x.elf / fixup4x.dat; Pi bootloader wants start4.elf / fixup4.dat
if [ -f "${FW_DIR}/start4x.elf" ] && [ ! -f "${FW_DIR}/start4.elf" ]; then
  cp -f "${FW_DIR}/start4x.elf" "${FW_DIR}/start4.elf"
fi
if [ -f "${FW_DIR}/fixup4x.dat" ] && [ ! -f "${FW_DIR}/fixup4.dat" ]; then
  cp -f "${FW_DIR}/fixup4x.dat" "${FW_DIR}/fixup4.dat"
fi

# 3) Provide kernel8.img (because config.txt uses the default filename)
# Buildroot produces Image; Raspberry Pi firmware expects kernel8.img by default on 64-bit
cp -f "${VMLINUX}" "${FW_DIR}/kernel8.img"

echo "post-image: staged config/cmdline + kernel8.img + start4/fixup4 in ${FW_DIR}"

e2label "${BINARIES_DIR}/data.ext4" data || true

# 4) Generate boot.vfat + sdcard.img with genimage
: "${HOST_DIR:?HOST_DIR not set}"
: "${BASE_DIR:?BASE_DIR not set}"
: "${TARGET_DIR:?TARGET_DIR not set}"

GENIMAGE_BIN="${HOST_DIR}/bin/genimage"
GENIMAGE_CFG="${BOOT_SRC}/genimage.cfg"
GENIMAGE_TMP="${BASE_DIR}/build/genimage.tmp"

[ -x "${GENIMAGE_BIN}" ] || { echo "ERROR: missing ${GENIMAGE_BIN} (host-genimage not built)"; exit 1; }
[ -f "${GENIMAGE_CFG}" ] || { echo "ERROR: missing ${GENIMAGE_CFG}"; exit 1; }

rm -f "${BINARIES_DIR}/boot.vfat" "${BINARIES_DIR}/sdcard.img"
rm -rf "${GENIMAGE_TMP}"
mkdir -p "${GENIMAGE_TMP}"

# Create persistent data partition image (8GiB ext4)
DATA_IMG="${BINARIES_DIR}/data.ext4"
rm -f "${DATA_IMG}"

# Fast sparse allocation instead of slow dd
truncate -s 8G "${DATA_IMG}"
mkfs.ext4 -F -L data "${DATA_IMG}" >/dev/null 2>&1
sync

"${GENIMAGE_BIN}" \
  --rootpath "${TARGET_DIR}" \
  --tmppath "${GENIMAGE_TMP}" \
  --inputpath "${BINARIES_DIR}" \
  --outputpath "${BINARIES_DIR}" \
  --config "${GENIMAGE_CFG}"

echo "post-image: generated boot.vfat and sdcard.img in ${BINARIES_DIR}"
