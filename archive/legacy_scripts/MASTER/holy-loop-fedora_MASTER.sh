#!/usr/bin/env bash
set -euo pipefail

DISK="/dev/sda"
IMAGE="$1"
UEFI_DIR="./rpi4uefi"

BOOT_SIZE="1024MiB"
ROOT_SIZE="1800GiB"
DATA_SIZE="2000GiB"

need() { command -v "$1" >/dev/null || { echo "Missing: $1"; exit 1; }; }
for c in parted wipefs mkfs.vfat mkfs.ext4 losetup rsync blkid awk sed grep mount umount chroot dracut btrfs; do need "$c"; done

[[ -b "$DISK" ]] || { echo "Disk not found: $DISK"; exit 1; }
[[ -f "$IMAGE" ]] || { echo "Image not found: $IMAGE"; exit 1; }
[[ -f "$UEFI_DIR/RPI_EFI.fd" ]] || { echo "Missing $UEFI_DIR/RPI_EFI.fd"; exit 1; }

echo ">>> SAFETY CHECK: ABOUT TO ERASE $DISK"
lsblk -o NAME,SIZE,MODEL "$DISK"
sleep 5

cleanup() {
  set +e
  umount -R /mnt/src 2>/dev/null || true
  umount -R /mnt/dst 2>/dev/null || true
  [[ -n "${LOOP:-}" ]] && losetup -d "$LOOP" 2>/dev/null || true
}
trap cleanup EXIT

echo ">>> Unmounting"
umount -R /mnt/src 2>/dev/null || true
umount -R /mnt/dst 2>/dev/null || true
for mp in $(lsblk -lnpo MOUNTPOINT "$DISK" | awk 'NF'); do umount -R "$mp" || true; done

echo ">>> Wiping disk"
wipefs -a "$DISK"
parted -s "$DISK" mklabel gpt

echo ">>> Creating partitions"
parted -s "$DISK" mkpart primary fat32 1MiB "$BOOT_SIZE"
parted -s "$DISK" set 1 esp on
parted -s "$DISK" mkpart primary ext4 "$BOOT_SIZE" "$ROOT_SIZE"
parted -s "$DISK" mkpart primary ext4 "$ROOT_SIZE" 100%

BOOTDEV="${DISK}1"
ROOTDEV="${DISK}2"
DATADEV="${DISK}3"

echo ">>> Formatting"
mkfs.vfat -F32 "$BOOTDEV"
mkfs.ext4 -F "$ROOTDEV"
mkfs.ext4 -F "$DATADEV"

mkdir -p /mnt/src/{efi,boot,root,root_sub} /mnt/dst/{boot,root,data}

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
mount "$DATADEV" /mnt/dst/data

echo ">>> Detect Fedora root subvol"
if btrfs subvolume list /mnt/src/root | awk '{print $NF}' | grep -qx root; then
  mount -t btrfs -o subvol=root "$ROOTSRC" /mnt/src/root_sub
  SRCROOT="/mnt/src/root_sub"
else
  SRCROOT="/mnt/src/root"
fi

echo ">>> Copying Fedora root"
rsync -aHAX --numeric-ids "$SRCROOT"/ /mnt/dst/root/

echo ">>> Copying Fedora EFI FIRST"
mkdir -p /mnt/dst/boot/EFI
rsync -rltDv --delete --no-owner --no-group --no-perms /mnt/src/efi/EFI/ /mnt/dst/boot/EFI/

echo ">>> Installing Pi4 UEFI LAST"
rsync -rltDv --delete --no-owner --no-group --no-perms "$UEFI_DIR"/ /mnt/dst/boot/

echo ">>> Bind mounts for dracut"
mount --bind /dev  /mnt/dst/root/dev
mount --bind /proc /mnt/dst/root/proc
mount --bind /sys  /mnt/dst/root/sys
mount --bind /tmp  /mnt/dst/root/tmp

echo ">>> Running dracut"
chroot /mnt/dst/root dracut -f

umount /mnt/dst/root/dev
umount /mnt/dst/root/proc
umount /mnt/dst/root/sys
umount /mnt/dst/root/tmp

echo ">>> Copy kernel + initramfs"
KERNEL=$(find /mnt/dst/root/usr/lib/modules -type f -name vmlinuz | head -n1)
INITRD=$(ls /mnt/dst/root/boot/initramfs-*.img | head -n1)

cp -av "$KERNEL" /mnt/dst/boot/kernel8.img
cp -av "$INITRD" /mnt/dst/boot/initramfs.img

ROOT_UUID=$(blkid -s UUID -o value "$ROOTDEV")

echo ">>> Writing config.txt"
cat > /mnt/dst/boot/config.txt <<EOF
arm_64bit=1
enable_uart=1
kernel=RPI_EFI.fd
EOF

echo ">>> Writing cmdline.txt"
cat > /mnt/dst/boot/cmdline.txt <<EOF
console=tty1 console=serial0,115200 root=UUID=$ROOT_UUID rootfstype=ext4 rw rootwait
EOF

echo ">>> fstab"
BOOT_UUID=$(blkid -s UUID -o value "$BOOTDEV")
DATA_UUID=$(blkid -s UUID -o value "$DATADEV")

cat >> /mnt/dst/root/etc/fstab <<EOF
UUID=$BOOT_UUID  /boot  vfat  defaults,noatime  0  2
UUID=$DATA_UUID  /data  ext4  defaults,noatime  0  2
EOF

echo ">>> Final sanity"
ls -lah /mnt/dst/boot/EFI/BOOT/BOOTAA64.EFI
ls -lah /mnt/dst/boot/RPI_EFI.fd
ls -lah /mnt/dst/boot/config.txt
ls -lah /mnt/dst/boot/cmdline.txt

sync
echo ">>> DONE"
