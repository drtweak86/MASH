#!/usr/bin/env bash
set -euo pipefail

DISK="/dev/sda"
IMAGE="${1:?Usage: $0 Fedora-*.raw[.xz]}"
UEFI_DIR="${UEFI_DIR:-./rpi4uefi}"   # must contain RPI_EFI.fd

BOOT_FAT_SIZE="1024MiB"   # sda1
BOOT_EXT_SIZE="2048MiB"   # sda2 (/boot)
ROOT_SIZE="1800GiB"       # sda3 (/)
# sda4 = rest (/data)

need() { command -v "$1" >/dev/null || { echo "Missing: $1"; exit 1; }; }
for c in parted wipefs mkfs.vfat mkfs.ext4 losetup rsync blkid awk sed grep findmnt btrfs dracut xz; do
  need "$c"
done

[[ -b "$DISK" ]] || { echo "Disk not found: $DISK"; exit 1; }
[[ -f "$IMAGE" ]] || { echo "Image not found: $IMAGE"; exit 1; }
[[ -f "$UEFI_DIR/RPI_EFI.fd" ]] || { echo "Missing $UEFI_DIR/RPI_EFI.fd"; exit 1; }

echo ">>> SAFETY CHECK: ABOUT TO ERASE $DISK"
lsblk -o NAME,SIZE,MODEL "$DISK"
echo "Ctrl+C now if wrong disk..."
sleep 5

TMPRAW=""
cleanup() {
  set +e
  umount -R /mnt/src 2>/dev/null || true
  umount -R /mnt/dst 2>/dev/null || true
  [[ -n "${LOOP:-}" ]] && losetup -d "$LOOP" 2>/dev/null || true
  [[ -n "$TMPRAW" ]] && rm -f "$TMPRAW" 2>/dev/null || true
}
trap cleanup EXIT

# .xz support
if [[ "$IMAGE" =~ \.xz$ ]]; then
  TMPRAW="$(mktemp --suffix=.raw)"
  echo ">>> Decompressing $IMAGE -> $TMPRAW"
  xz -dc "$IMAGE" > "$TMPRAW"
  IMAGE="$TMPRAW"
fi

echo ">>> Unmounting anything using $DISK"
for mp in $(lsblk -lnpo MOUNTPOINT "$DISK" | awk 'NF'); do umount -R "$mp" 2>/dev/null || true; done

echo ">>> Wiping disk signatures"
wipefs -a "$DISK" || true

echo ">>> Creating GPT partition table"
parted -s "$DISK" mklabel gpt

echo ">>> Creating partitions"
# sda1 FAT32 (ESP/Pi boot)
parted -s "$DISK" mkpart primary fat32 1MiB "$BOOT_FAT_SIZE"
parted -s "$DISK" set 1 esp on

# sda2 ext4 /boot (2GiB)
parted -s "$DISK" mkpart primary ext4 "$BOOT_FAT_SIZE" "$BOOT_EXT_SIZE"

# sda3 ext4 / (1800GiB)
parted -s "$DISK" mkpart primary ext4 "$BOOT_EXT_SIZE" "$ROOT_SIZE"

# sda4 ext4 /data (rest)
parted -s "$DISK" mkpart primary ext4 "$ROOT_SIZE" 100%

parted -s "$DISK" print

BOOTDEV="${DISK}1"
BOOTEXT="${DISK}2"
ROOTDEV="${DISK}3"
DATADEV="${DISK}4"

echo ">>> Formatting filesystems"
mkfs.vfat -F32 -n EFI "$BOOTDEV"
mkfs.ext4 -F -L BOOT "$BOOTEXT"
mkfs.ext4 -F -L ROOT "$ROOTDEV"
mkfs.ext4 -F -L DATA "$DATADEV"

mkdir -p /mnt/src/{efi,boot,root,root_sub} /mnt/dst/{efi,boot,root}

echo ">>> Loop mounting Fedora image"
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
mount "$BOOTEXT" /mnt/dst/boot
mount "$ROOTDEV" /mnt/dst/root

echo ">>> Selecting Fedora btrfs root (prefer subvol=root)"
SRCROOT="/mnt/src/root"
if btrfs subvolume list /mnt/src/root | awk '{print $NF}' | grep -qx root; then
  mount -t btrfs -o subvol=root "$ROOTSRC" /mnt/src/root_sub
  SRCROOT="/mnt/src/root_sub"
  echo "Using subvol=root"
else
  echo "No subvol=root found; using top-level btrfs"
fi

echo ">>> Copying Fedora root -> ext4 root (progress bar)"
rsync -aHAX --numeric-ids --info=progress2 "$SRCROOT"/ /mnt/dst/root/

echo ">>> Copying Fedora /boot partition -> destination /boot"
rsync -aHAX --info=progress2 /mnt/src/boot/ /mnt/dst/boot/

echo ">>> Installing Fedora EFI files (EFI/*) onto FAT"
mkdir -p /mnt/dst/efi/EFI
rsync -a --delete --no-owner --no-group --no-perms /mnt/src/efi/EFI/ /mnt/dst/efi/EFI/

echo ">>> Installing Pi4 UEFI firmware LAST (PFTF) onto FAT"
rsync -a --delete --no-owner --no-group --no-perms "$UEFI_DIR"/ /mnt/dst/efi/

echo ">>> Writing Pi4 UEFI config.txt"
cat > /mnt/dst/efi/config.txt <<'EOF'
arm_64bit=1
enable_uart=1
dtoverlay=upstream-pi4
kernel=RPI_EFI.fd
EOF

echo ">>> Ensure mountpoints and fstab entries"
mkdir -p /mnt/dst/root/boot /mnt/dst/root/boot/efi /mnt/dst/root/data
BOOTUUID="$(blkid -s UUID -o value "$BOOTEXT")"
EFIUUID="$(blkid -s UUID -o value "$BOOTDEV")"
ROOTUUID="$(blkid -s UUID -o value "$ROOTDEV")"
DATAUUID="$(blkid -s UUID -o value "$DATADEV")"

# Write a clean fstab (you can tweak later)
cat > /mnt/dst/root/etc/fstab <<EOF
UUID=$ROOTUUID  /         ext4  defaults,noatime  0  1
UUID=$BOOTUUID  /boot     ext4  defaults,noatime  0  2
UUID=$EFIUUID   /boot/efi vfat  umask=0077,shortname=winnt  0  2
UUID=$DATAUUID  /data     ext4  defaults,noatime  0  2
EOF

echo ">>> Bind mounts for dracut (needs /boot mounted!)"
mkdir -p /mnt/dst/root/{dev,proc,sys,run,tmp,var/tmp}
chmod 1777 /mnt/dst/root/tmp /mnt/dst/root/var/tmp || true

mount --bind /dev  /mnt/dst/root/dev
mount --bind /proc /mnt/dst/root/proc
mount --bind /sys  /mnt/dst/root/sys
mount --bind /run  /mnt/dst/root/run
mkdir -p /mnt/dst/root/dev/pts
mount -t devpts devpts /mnt/dst/root/dev/pts

# Mount the real /boot and /boot/efi inside the chroot
mount --bind /mnt/dst/boot /mnt/dst/root/boot
mount --bind /mnt/dst/efi  /mnt/dst/root/boot/efi

echo ">>> Running dracut in chroot (regenerate all)"
chroot /mnt/dst/root dracut -f --regenerate-all

echo ">>> Unmount chroot binds"
umount /mnt/dst/root/boot/efi || true
umount /mnt/dst/root/boot || true
umount /mnt/dst/root/dev/pts || true
umount /mnt/dst/root/dev || true
umount /mnt/dst/root/proc || true
umount /mnt/dst/root/sys || true
umount /mnt/dst/root/run || true

echo ">>> Final sanity"
ls -lah /mnt/dst/efi/RPI_EFI.fd /mnt/dst/efi/start4.elf /mnt/dst/efi/fixup4.dat
ls -lah /mnt/dst/efi/EFI/BOOT/BOOTAA64.EFI
echo "--- config.txt ---"; cat /mnt/dst/efi/config.txt
echo "--- fstab ---"; head -n 20 /mnt/dst/root/etc/fstab
sync

echo ">>> Unmounting"
umount -R /mnt/dst || true
umount -R /mnt/src || true
losetup -d "$LOOP" || true

echo "✅ Done. Boot Pi4 from MASH using UEFI."
