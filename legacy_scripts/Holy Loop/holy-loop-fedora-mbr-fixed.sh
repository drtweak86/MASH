#!/usr/bin/env bash
set -euo pipefail

DISK="/dev/sda"
IMAGE="$1"
UEFI_DIR="./rpi4uefi"   # must contain RPI_EFI.fd etc

BOOT_SIZE="1024MiB"
ROOT_SIZE="1800GiB"
DATA_SIZE="2000GiB"

need() { command -v "$1" >/dev/null || { echo "Missing: $1"; exit 1; }; }
for c in parted wipefs mkfs.vfat mkfs.ext4 losetup rsync blkid awk sed grep btrfs pv; do need "$c"; done

[[ -b "$DISK" ]] || { echo "Disk not found: $DISK"; exit 1; }
[[ -f "$IMAGE" ]] || { echo "Image not found: $IMAGE"; exit 1; }
[[ -f "$UEFI_DIR/RPI_EFI.fd" ]] || { echo "Missing $UEFI_DIR/RPI_EFI.fd"; exit 1; }

echo ">>> ABOUT TO ERASE $DISK"
lsblk -o NAME,SIZE,MODEL "$DISK"
sleep 5

cleanup() {
  set +e
  umount -R /mnt/src 2>/dev/null
  umount -R /mnt/dst 2>/dev/null
  [[ -n "${LOOP:-}" ]] && losetup -d "$LOOP" 2>/dev/null
}
trap cleanup EXIT

echo ">>> Unmounting old mounts"
for mp in $(lsblk -lnpo MOUNTPOINT "$DISK" | awk 'NF'); do umount -R "$mp" 2>/dev/null || true; done

echo ">>> Wiping disk"
wipefs -a "$DISK"

echo ">>> Creating MBR partition table"
parted -s "$DISK" mklabel msdos
parted -s "$DISK" mkpart primary fat32 1MiB "$BOOT_SIZE"
parted -s "$DISK" set 1 boot on
parted -s "$DISK" mkpart primary ext4 "$BOOT_SIZE" "$ROOT_SIZE"
parted -s "$DISK" mkpart primary ext4 "$ROOT_SIZE" 100%
parted -s "$DISK" print

BOOTDEV="${DISK}1"
ROOTDEV="${DISK}2"
DATADEV="${DISK}3"

echo ">>> Formatting filesystems"
mkfs.vfat -F32 "$BOOTDEV"
mkfs.ext4 -F "$ROOTDEV"
mkfs.ext4 -F "$DATADEV"

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

echo ">>> Finding Fedora root subvolume"
if btrfs subvolume list /mnt/src/root | awk '{print $NF}' | grep -qx "root"; then
  mount -t btrfs -o subvol=root "$ROOTSRC" /mnt/src/root_sub
  SRCROOT="/mnt/src/root_sub"
else
  SRCROOT="/mnt/src/root"
fi

echo ">>> Copying Fedora root filesystem (progress bar)"
tar -C "$SRCROOT" -cf - . | pv | tar -C /mnt/dst/root -xf -

echo ">>> Installing Pi4 UEFI firmware"
rsync -rtv "$UEFI_DIR"/ /mnt/dst/boot/

echo ">>> Copying Fedora EFI loaders into /EFI"
mkdir -p /mnt/dst/boot/EFI
rsync -rtv /mnt/src/efi/EFI/ /mnt/dst/boot/EFI/

echo ">>> Writing config.txt"
cat > /mnt/dst/boot/config.txt <<EOF
arm_64bit=1
enable_uart=1
kernel=RPI_EFI.fd
EOF

echo ">>> Writing cmdline.txt (blank for UEFI)"
: > /mnt/dst/boot/cmdline.txt

echo ">>> Fixing fstab"
BOOT_UUID=$(blkid -s UUID -o value "$BOOTDEV")
ROOT_UUID=$(blkid -s UUID -o value "$ROOTDEV")
DATA_UUID=$(blkid -s UUID -o value "$DATADEV")

sed -i '/ \/boot /d' /mnt/dst/root/etc/fstab || true
sed -i '/ \/data /d' /mnt/dst/root/etc/fstab || true

echo "UUID=$BOOT_UUID  /boot  vfat  defaults  0  2" >> /mnt/dst/root/etc/fstab
echo "UUID=$DATA_UUID  /data  ext4  defaults  0  2" >> /mnt/dst/root/etc/fstab

echo ">>> Final sanity check"
ls -lah /mnt/dst/boot/RPI_EFI.fd
ls -lah /mnt/dst/boot/start4.elf
ls -lah /mnt/dst/boot/EFI/BOOT/BOOTAA64.EFI

sync
echo "✅ DONE — remove drive and boot Pi 4"
