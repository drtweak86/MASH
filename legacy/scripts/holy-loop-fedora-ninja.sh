#!/usr/bin/env bash
set -euo pipefail

DISK="/dev/sda"
IMAGE="${1:-}"
UEFI_DIR="./rpi4uefi"

EFI_START="4MiB"
EFI_END="1024MiB"
BOOT_END="3072MiB"
ROOT_END="70%"
MAKE_DATA="yes"

# NINJA knobs
NICE_LVL="${NICE_LVL:-10}"        # 0..19 (higher = nicer)
IONICE_CLASS="${IONICE_CLASS:-2}" # 2=best-effort
IONICE_LVL="${IONICE_LVL:-7}"     # 0..7
RSYNC_JOBS="${RSYNC_JOBS:-1}"     # keep 1 unless you *know* your storage benefits
QUIET="${QUIET:-0}"               # 1 = quieter rsync (still shows progress)

need() { command -v "$1" >/dev/null 2>&1 || { echo "Missing: $1" >&2; exit 1; }; }

for c in parted wipefs mkfs.vfat mkfs.ext4 losetup rsync blkid awk sed grep findmnt btrfs dracut file; do need "$c"; done
need pv || true
need xz || true

[[ -n "$IMAGE" ]] || { echo "Usage: sudo $0 <Fedora-*.raw|.raw.xz>" >&2; exit 1; }
[[ -b "$DISK" ]]  || { echo "Disk not found: $DISK" >&2; exit 1; }
[[ -f "$IMAGE" ]] || { echo "Image not found: $IMAGE" >&2; exit 1; }
[[ -f "$UEFI_DIR/RPI_EFI.fd" ]] || { echo "Missing $UEFI_DIR/RPI_EFI.fd" >&2; exit 1; }
[[ $EUID -eq 0 ]] || { echo "Run with sudo." >&2; exit 1; }

step=""
log()  { echo -e "\n>>> $*"; }
die()  { echo -e "\n[FATAL] Step failed: $step" >&2; exit 1; }
trap die ERR

# Prefer gentle I/O priority
wrap_io() {
  if command -v ionice >/dev/null 2>&1; then
    ionice -c "$IONICE_CLASS" -n "$IONICE_LVL" nice -n "$NICE_LVL" "$@"
  else
    nice -n "$NICE_LVL" "$@"
  fi
}

cleanup() {
  set +e
  umount -R /mnt/dst/root/boot/efi 2>/dev/null || true
  umount -R /mnt/dst/root/boot     2>/dev/null || true
  umount -R /mnt/dst/root/data     2>/dev/null || true

  umount -R /mnt/src/efi 2>/dev/null || true
  umount -R /mnt/src/boot 2>/dev/null || true
  umount -R /mnt/src/root_sub 2>/dev/null || true
  umount -R /mnt/src/root 2>/dev/null || true

  umount -R /mnt/dst/efi 2>/dev/null || true
  umount -R /mnt/dst/boot 2>/dev/null || true
  umount -R /mnt/dst/root 2>/dev/null || true
  umount -R /mnt/dst/data 2>/dev/null || true

  [[ -n "${LOOP:-}" ]] && losetup -d "$LOOP" 2>/dev/null || true
  [[ -n "${TMPRAW:-}" ]] && rm -f "$TMPRAW" 2>/dev/null || true
}
trap cleanup EXIT

log "SAFETY CHECK: ABOUT TO ERASE $DISK"
lsblk -o NAME,SIZE,MODEL "$DISK"
echo "Ctrl+C in 5 seconds..."
sleep 5

step="Unmounting"
log "Unmounting anything using $DISK"
for mp in $(lsblk -lnpo MOUNTPOINT "$DISK" | awk 'NF'); do umount -R "$mp" 2>/dev/null || true; done
umount -R /mnt/src 2>/dev/null || true
umount -R /mnt/dst 2>/dev/null || true

step="Wipe signatures"
log "Wiping signatures"
wipefs -a "$DISK" || true

step="Decompress (optional)"
TMPRAW=""
if [[ "$IMAGE" =~ \.xz$ ]]; then
  log "Decompressing .xz -> temp raw (with pv)"
  TMPRAW="$(mktemp --suffix=.raw)"
  if command -v pv >/dev/null 2>&1; then
    xz -dc "$IMAGE" | pv > "$TMPRAW"
  else
    xz -dc "$IMAGE" > "$TMPRAW"
  fi
  IMAGE="$TMPRAW"
fi

step="Partition disk (MBR + flags)"
log "Partitioning (MBR/msdos + boot+lba flags)"
parted -s "$DISK" mklabel msdos
parted -s "$DISK" mkpart primary fat32 "$EFI_START" "$EFI_END"
parted -s "$DISK" set 1 boot on
parted -s "$DISK" set 1 lba on
parted -s "$DISK" mkpart primary ext4 "$EFI_END" "$BOOT_END"
parted -s "$DISK" mkpart primary ext4 "$BOOT_END" "$ROOT_END"
if [[ "$MAKE_DATA" == "yes" ]]; then
  parted -s "$DISK" mkpart primary ext4 "$ROOT_END" 100%
fi
parted -s "$DISK" unit MiB print

EFI_DEV="${DISK}1"
BOOT_DEV="${DISK}2"
ROOT_DEV="${DISK}3"
DATA_DEV=""
[[ "$MAKE_DATA" == "yes" ]] && DATA_DEV="${DISK}4"

step="Make filesystems"
log "Formatting"
mkfs.vfat -F 32 -n EFI "$EFI_DEV"
mkfs.ext4 -F -L BOOT "$BOOT_DEV"
mkfs.ext4 -F -L ROOT "$ROOT_DEV"
[[ -n "$DATA_DEV" ]] && mkfs.ext4 -F -L DATA "$DATA_DEV"

mkdir -p /mnt/src/{efi,boot,root,root_sub} /mnt/dst/{efi,boot,root,data}

step="Loop mount image"
log "Loop-mounting Fedora image"
LOOP="$(losetup --show -Pf "$IMAGE")"
lsblk "$LOOP"
IMG_EFI="${LOOP}p1"
IMG_BOOT="${LOOP}p2"
IMG_ROOT="${LOOP}p3"

step="Mount image + destination"
log "Mounting image partitions"
mount "$IMG_EFI"  /mnt/src/efi
mount "$IMG_BOOT" /mnt/src/boot
mount -t btrfs "$IMG_ROOT" /mnt/src/root

log "Mounting destination partitions"
mount "$EFI_DEV"  /mnt/dst/efi
mount "$BOOT_DEV" /mnt/dst/boot
mount "$ROOT_DEV" /mnt/dst/root
[[ -n "$DATA_DEV" ]] && mount "$DATA_DEV" /mnt/dst/data

step="Select btrfs root subvol"
log "Selecting Fedora btrfs root (prefer subvol=root)"
SRCROOT="/mnt/src/root"
if btrfs subvolume list /mnt/src/root | awk '{print $NF}' | grep -qx root; then
  mount -t btrfs -o subvol=root "$IMG_ROOT" /mnt/src/root_sub
  SRCROOT="/mnt/src/root_sub"
fi

# rsync tuning
RSYNC_BASE=(-aHAX --numeric-ids --whole-file --inplace --info=progress2)
[[ "$QUIET" == "1" ]] && RSYNC_BASE+=(-q)

step="Copy root"
log "Copying Fedora root -> / (progress)"
wrap_io rsync "${RSYNC_BASE[@]}" "$SRCROOT"/ /mnt/dst/root/

step="Copy boot"
log "Copying Fedora /boot -> real /boot (for dracut + grub)"
wrap_io rsync "${RSYNC_BASE[@]}" /mnt/src/boot/ /mnt/dst/boot/

step="Bind mount boot/efi + data into target root"
log "Mounting /boot + /boot/efi inside target root"
mkdir -p /mnt/dst/root/boot /mnt/dst/root/boot/efi
mount --bind /mnt/dst/boot /mnt/dst/root/boot
mount --bind /mnt/dst/efi  /mnt/dst/root/boot/efi
if [[ -n "$DATA_DEV" ]]; then
  mkdir -p /mnt/dst/root/data
  mount --bind /mnt/dst/data /mnt/dst/root/data
fi

step="Copy Fedora EFI + PFTF UEFI"
log "Installing Fedora EFI files (EFI/*) onto EFI partition"
mkdir -p /mnt/dst/efi/EFI
rsync -aHAX /mnt/src/efi/EFI/ /mnt/dst/efi/EFI/

log "Installing Pi4 UEFI firmware LAST (PFTF) onto EFI partition"
rsync -rltD --no-owner --no-group --no-perms "$UEFI_DIR"/ /mnt/dst/efi/

step="Write config.txt (PFTF)"
log "Writing Pi4 UEFI config.txt (PFTF)"
cat > /mnt/dst/efi/config.txt <<'EOF'
arm_64bit=1
enable_uart=1
enable_gic=1
armstub=RPI_EFI.fd
disable_commandline_tags=2
device_tree_address=0x3e0000
device_tree_end=0x400000
dtoverlay=upstream-pi4
EOF

step="Write fstab"
log "Writing UUID fstab"
EFI_UUID="$(blkid -s UUID -o value "$EFI_DEV")"
BOOT_UUID="$(blkid -s UUID -o value "$BOOT_DEV")"
ROOT_UUID="$(blkid -s UUID -o value "$ROOT_DEV")"
DATA_UUID=""
[[ -n "$DATA_DEV" ]] && DATA_UUID="$(blkid -s UUID -o value "$DATA_DEV")"

cat > /mnt/dst/root/etc/fstab <<EOF
UUID=$ROOT_UUID  /         ext4  defaults,noatime  0 1
UUID=$BOOT_UUID  /boot     ext4  defaults,noatime  0 2
UUID=$EFI_UUID   /boot/efi vfat  umask=0077        0 2
EOF
[[ -n "$DATA_UUID" ]] && echo "UUID=$DATA_UUID  /data     ext4  defaults,noatime  0 2" >> /mnt/dst/root/etc/fstab

step="Dracut chroot prereqs"
log "Bind mounts for dracut (+ fix /var/tmp + devpts)"
mkdir -p /mnt/dst/root/var/tmp /mnt/dst/root/run /mnt/dst/root/dev/pts
chmod 1777 /mnt/dst/root/tmp 2>/dev/null || true
chmod 1777 /mnt/dst/root/var/tmp 2>/dev/null || true

mount --bind /dev  /mnt/dst/root/dev
mount --bind /proc /mnt/dst/root/proc
mount --bind /sys  /mnt/dst/root/sys
mount --bind /run  /mnt/dst/root/run
mount -t devpts devpts /mnt/dst/root/dev/pts

step="Run dracut"
log "Running dracut in chroot (regenerate all)"
chroot /mnt/dst/root dracut --regenerate-all --force

step="Optional grub config"
log "Optional: grub2-mkconfig (ignore failures)"
chroot /mnt/dst/root grub2-mkconfig -o /boot/grub2/grub.cfg 2>/dev/null || true

step="Unmount chroot binds"
log "Unmounting chroot binds"
umount /mnt/dst/root/dev/pts || true
umount /mnt/dst/root/run || true
umount /mnt/dst/root/sys || true
umount /mnt/dst/root/proc || true
umount /mnt/dst/root/dev || true

step="Sanity checks"
log "Sanity checks"
echo "Disk should be DOS + sda1 W95 FAT32 (LBA) + Boot *"
command -v fdisk >/dev/null 2>&1 && fdisk -l "$DISK" | sed -n '1,120p'

ls -lah /mnt/dst/efi/start4.elf /mnt/dst/efi/fixup4.dat /mnt/dst/efi/RPI_EFI.fd
ls -lah /mnt/dst/efi/EFI/BOOT/BOOTAA64.EFI
echo "config.txt:"
cat /mnt/dst/efi/config.txt | cat -A

log "Done. Sync + unmount."
sync
echo "✅  NINJA DONE. Power off, move MASH to the Pi 4, boot via UEFI -> Fedora."
