#!/usr/bin/env bash
set -euo pipefail

DISK="/dev/sda"
IMAGE="$1"
UEFI_DIR="./rpi4uefi"   # must contain RPI_EFI.fd, start4.elf, fixup4.dat, overlays/

BOOT_SIZE="1024MiB"

need() { command -v "$1" >/dev/null || { echo "Missing: $1"; exit 1; }; }
for c in parted wipefs mkfs.vfat mkfs.ext4 losetup rsync blkid awk sed grep btrfs; do need "$c"; done

[[ -b "$DISK" ]] || { echo "Disk not found: $DISK"; exit 1; }
[[ -f "$IMAGE" ]] || { echo "Image not found: $IMAGE"; exit 1; }
[[ -f "$UEFI_DIR/RPI_EFI.fd" ]] || { echo "Missing $UEFI_DIR/RPI_EFI.fd"; exit 1; }

echo ">>> SAFETY CHECK: ABOUT TO ERASE $DISK"
lsblk -o NAME,SIZE,MODEL "$DISK"
sleep 5

cleanup() {
  set +e
  umount -R /mnt/src 2>/dev/null
  umount -R /mnt/dst 2>/dev/null
  [[ -n "${LOOP:-}" ]] && losetup -d "$LOOP" 2>/dev/null
}
trap cleanup EXIT

echo ">>> Wiping disk"
wipefs -a "$DISK" || true
parted -s "$DISK" mklabel gpt
parted -s "$DISK" mkpart primary fat32 1MiB "$BOOT_SIZE"
parted -s "$DISK" set 1 esp on
parted -s "$DISK" mkpart primary ext4 "$BOOT_SIZE" 100%

BOOTDEV="${DISK}1"
ROOTDEV="${DISK}2"

echo ">>> Formatting"
mkfs.vfat -F32 "$BOOTDEV"
mkfs.ext4 -F "$ROOTDEV"

mkdir -p /mnt/src/{efi,boot,root,root_sub} /mnt/dst/{boot,root}

echo ">>> Loop mounting image"
LOOP=$(losetup --show -Pf "$IMAGE")
lsblk "$LOOP"

EFISRC="${LOOP}p1"
BOOTSRC="${LOOP}p2"
ROOTSRC="${LOOP}p3"

mount "$EFISRC" /mnt/src/efi
mount "$BOOTSRC" /mnt/src/boot
mount -t btrfs "$ROOTSRC" /mnt/src/root

mount "$BOOTDEV" /mnt/dst/boot
mount "$ROOTDEV" /mnt/dst/root

echo ">>> Copying Fedora root"
if btrfs subvolume list /mnt/src/root | awk '{print $NF}' | grep -qx root; then
  mount -t btrfs -o subvol=root "$ROOTSRC" /mnt/src/root_sub
  rsync -aHAX --numeric-ids /mnt/src/root_sub/ /mnt/dst/root/
  umount /mnt/src/root_sub
else
  rsync -aHAX --numeric-ids /mnt/src/root/ /mnt/dst/root/
fi

echo ">>> Copying Fedora EFI first"
mkdir -p /mnt/dst/boot/EFI
rsync -rtv /mnt/src/efi/EFI/ /mnt/dst/boot/EFI/

echo ">>> Installing Pi4 UEFI LAST"
rsync -rtv "$UEFI_DIR"/ /mnt/dst/boot/

echo ">>> Writing config.txt"
cat > /mnt/dst/boot/config.txt <<EOF
arm_64bit=1
enable_uart=1
kernel=RPI_EFI.fd
EOF

ROOT_UUID="$(blkid -s UUID -o value "$ROOTDEV")"

echo ">>> Writing cmdline.txt"
cat > /mnt/dst/boot/cmdline.txt <<EOF
console=tty1 console=serial0,115200 root=UUID=$ROOT_UUID rootfstype=ext4 rw rootwait
EOF

echo ">>> Ensuring fstab"
BOOT_UUID="$(blkid -s UUID -o value "$BOOTDEV")"
mkdir -p /mnt/dst/root/boot
grep -q "$BOOT_UUID" /mnt/dst/root/etc/fstab || \
echo "UUID=$BOOT_UUID /boot vfat defaults,noatime 0 2" >> /mnt/dst/root/etc/fstab

echo ">>> Final sanity"
ls -lah /mnt/dst/boot/RPI_EFI.fd /mnt/dst/boot/start4.elf /mnt/dst/boot/EFI/BOOT/BOOTAA64.EFI

sync
umount -R /mnt/dst
umount -R /mnt/src
losetup -d "$LOOP"

echo "✅ DONE — UEFI written LAST. No more rainbow square sabotage."
