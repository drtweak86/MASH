#!/usr/bin/env bash
set -euo pipefail

DISK="/dev/sda"                       # MASH
IMAGE="${1:-}"                        # Fedora-KDE-Desktop-Disk-43-1.6.aarch64.raw
UEFI_DIR="./rpi4uefi"                 # must contain RPI_EFI.fd, start4.elf, fixup4.dat, overlays/, firmware/

BOOT_SIZE_MIB=1024                    # 1GiB FAT
ROOT_SIZE_GIB=1800                    # 1.8TiB root (so msdos limit not hit per-partition)
# remainder becomes DATA (about 1.8TiB on a 4TB disk)
MAKE_DATA="yes"

need() { command -v "$1" >/dev/null || { echo "Missing: $1"; exit 1; }; }
for c in parted wipefs mkfs.vfat mkfs.ext4 losetup rsync blkid awk sed grep findmnt btrfs dracut udevadm partprobe file; do need "$c"; done

[[ $EUID -eq 0 ]] || { echo "Run as root: sudo $0 <Fedora.raw>"; exit 1; }
[[ -b "$DISK" ]] || { echo "Disk not found: $DISK"; exit 1; }
[[ -n "$IMAGE" && -f "$IMAGE" ]] || { echo "Usage: $0 Fedora-*.raw"; exit 1; }
[[ -f "$UEFI_DIR/RPI_EFI.fd" ]] || { echo "Missing $UEFI_DIR/RPI_EFI.fd (extract the PFTF zip to ./rpi4uefi)"; exit 1; }

cleanup() {
  set +e
  umount -R /mnt/src 2>/dev/null || true
  umount -R /mnt/dst 2>/dev/null || true
  umount -R /mnt/dst/root/dev 2>/dev/null || true
  umount -R /mnt/dst/root/proc 2>/dev/null || true
  umount -R /mnt/dst/root/sys 2>/dev/null || true
  umount -R /mnt/dst/root/tmp 2>/dev/null || true
  umount -R /mnt/dst/root/dev/pts 2>/dev/null || true
  [[ -n "${LOOP:-}" ]] && losetup -d "$LOOP" 2>/dev/null || true
}
trap cleanup EXIT

echo ">>> SAFETY CHECK: ABOUT TO ERASE $DISK"
lsblk -o NAME,SIZE,MODEL "$DISK"
echo "Ctrl+C in 5 seconds if wrong disk..."
sleep 5

echo ">>> Unmounting anything using $DISK"
for mp in $(lsblk -lnpo MOUNTPOINT "$DISK" | awk 'NF'); do umount -R "$mp" 2>/dev/null || true; done

echo ">>> Wiping signatures"
wipefs -a "$DISK" || true

echo ">>> Creating MBR (msdos) partition table + correct flags"
parted -s "$DISK" mklabel msdos

# Partition 1: FAT32 (LBA) for Pi boot/UEFI
# Use MiB alignment; start at 4MiB to avoid any weirdness.
parted -s -a optimal "$DISK" mkpart primary fat32 4MiB "$((BOOT_SIZE_MIB))MiB"
parted -s "$DISK" set 1 boot on
parted -s "$DISK" set 1 lba on

# Partition 2: ext4 ROOT, ends at ROOT_SIZE_GIB
parted -s -a optimal "$DISK" mkpart primary ext4 "$((BOOT_SIZE_MIB))MiB" "${ROOT_SIZE_GIB}GiB"

# Partition 3: ext4 DATA (rest of disk)
if [[ "$MAKE_DATA" == "yes" ]]; then
  parted -s -a optimal "$DISK" mkpart primary ext4 "${ROOT_SIZE_GIB}GiB" 100%
fi

partprobe "$DISK" || true
udevadm settle || true

echo ">>> Partition table:"
parted -s "$DISK" unit MiB print

BOOTDEV="${DISK}1"
ROOTDEV="${DISK}2"
DATADEV="${DISK}3"

echo ">>> Formatting filesystems"
mkfs.vfat -F32 -n PI_BOOT "$BOOTDEV"
mkfs.ext4 -F -L FEDORA_ROOT "$ROOTDEV"
if [[ "$MAKE_DATA" == "yes" ]]; then
  mkfs.ext4 -F -L DATA "$DATADEV"
fi

mkdir -p /mnt/src/{efi,boot,root,root_sub} /mnt/dst/{efi,root}

echo ">>> Loop-mounting Fedora image"
LOOP=$(losetup --show -Pf "$IMAGE")
lsblk "$LOOP"

EFISRC="${LOOP}p1"
BOOTSRC="${LOOP}p2"
ROOTSRC="${LOOP}p3"

echo ">>> Mounting image partitions"
mount "$EFISRC" /mnt/src/efi
mount "$BOOTSRC" /mnt/src/boot
mount -t btrfs "$ROOTSRC" /mnt/src/root

echo ">>> Mounting destination partitions"
mount "$BOOTDEV" /mnt/dst/efi
mount "$ROOTDEV" /mnt/dst/root

echo ">>> Selecting Fedora btrfs root (prefer subvol=root)"
if btrfs subvolume list /mnt/src/root | awk '{print $NF}' | grep -qx root; then
  echo "Using subvol=root"
  mount -t btrfs -o subvol=root "$ROOTSRC" /mnt/src/root_sub
  SRCROOT="/mnt/src/root_sub"
else
  echo "No subvol=root; using top-level"
  SRCROOT="/mnt/src/root"
fi

echo ">>> Copying Fedora root -> ext4 root (progress bar)"
rsync -aHAX --numeric-ids --info=progress2 "$SRCROOT"/ /mnt/dst/root/

echo ">>> Copying Fedora /boot partition -> destination /boot (so dracut has a real /boot)"
mkdir -p /mnt/dst/root/boot
rsync -aHAX --numeric-ids --info=progress2 /mnt/src/boot/ /mnt/dst/root/boot/

echo ">>> Installing Fedora EFI files (EFI/*) onto FAT"
mkdir -p /mnt/dst/efi/EFI
rsync -a /mnt/src/efi/EFI/ /mnt/dst/efi/EFI/

echo ">>> Installing Pi4 UEFI firmware LAST (PFTF) onto FAT"
# FAT can’t chown; avoid rsync ownership/attrs that cause code 23
rsync -a --no-owner --no-group --no-perms "$UEFI_DIR"/ /mnt/dst/efi/

echo ">>> Writing Pi4 UEFI config.txt (important: dtoverlay=upstream-pi4)"
cat > /mnt/dst/efi/config.txt <<'EOF'
arm_64bit=1
enable_uart=1
dtoverlay=upstream-pi4
kernel=RPI_EFI.fd
EOF

echo ">>> Ensure mountpoints + fstab entries"
BOOT_UUID="$(blkid -s UUID -o value "$BOOTDEV")"
mkdir -p /mnt/dst/root/boot/efi
# Add /boot/efi entry (mount FAT at runtime)
grep -q "UUID=$BOOT_UUID" /mnt/dst/root/etc/fstab 2>/dev/null || \
  echo "UUID=$BOOT_UUID  /boot/efi  vfat  defaults,umask=0077,shortname=winnt  0  0" >> /mnt/dst/root/etc/fstab

if [[ "$MAKE_DATA" == "yes" ]]; then
  DATA_UUID="$(blkid -s UUID -o value "$DATADEV")"
  mkdir -p /mnt/dst/root/data
  grep -q "UUID=$DATA_UUID" /mnt/dst/root/etc/fstab 2>/dev/null || \
    echo "UUID=$DATA_UUID  /data  ext4  defaults,noatime  0  2" >> /mnt/dst/root/etc/fstab
fi

echo ">>> Bind mounts for dracut (and fix /var/tmp + devpts)"
mkdir -p /mnt/dst/root/{dev,proc,sys,tmp,var/tmp,dev/pts}
chmod 1777 /mnt/dst/root/tmp /mnt/dst/root/var/tmp

mount --bind /dev  /mnt/dst/root/dev
mount --bind /proc /mnt/dst/root/proc
mount --bind /sys  /mnt/dst/root/sys
mount --bind /tmp  /mnt/dst/root/tmp
mount -t devpts devpts /mnt/dst/root/dev/pts

# IMPORTANT: mount the FAT as /boot/efi inside chroot for EFI tooling (harmless even if not used now)
mount --bind /mnt/dst/efi /mnt/dst/root/boot/efi

echo ">>> Running dracut in chroot (regenerate all)"
chroot /mnt/dst/root dracut -f --regenerate-all

echo ">>> Unmount chroot binds"
umount -R /mnt/dst/root/boot/efi || true
umount -R /mnt/dst/root/dev/pts || true
umount -R /mnt/dst/root/tmp || true
umount -R /mnt/dst/root/sys || true
umount -R /mnt/dst/root/proc || true
umount -R /mnt/dst/root/dev || true

echo ">>> Final sanity (FAT must be readable by Pi ROM)"
echo "Disklabel should be DOS + sda1 should be W95 FAT32 (LBA) + Boot *"
fdisk -l "$DISK" | sed -n '1,120p'
echo
ls -lah /mnt/dst/efi/start4.elf /mnt/dst/efi/fixup4.dat /mnt/dst/efi/RPI_EFI.fd
ls -lah /mnt/dst/efi/EFI/BOOT/BOOTAA64.EFI
echo
echo "config.txt:"
cat /mnt/dst/efi/config.txt | cat -A

sync
echo "✅ Done. Power off, move MASH to the Pi, boot."
