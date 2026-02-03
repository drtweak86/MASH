#!/usr/bin/env bash
set -euo pipefail

# ====== CONFIG ======
DISK="/dev/sda"                          # MASH (PS4 Game Drive)
IMAGE="${1:?Usage: $0 Fedora-*.raw[.xz]}" # Fedora raw image (or raw.xz)
UEFI_DIR="${UEFI_DIR:-./rpi4uefi}"       # extracted PFTF zip dir containing RPI_EFI.fd

BOOT_SIZE="1024MiB"   # FAT32 ESP/boot (Pi reads this)
ROOT_SIZE="1800GiB"   # ext4 root size
# DATA will be "rest of disk" automatically
# ====================

need() { command -v "$1" >/dev/null || { echo "Missing: $1"; exit 1; }; }
for c in parted wipefs mkfs.vfat mkfs.ext4 losetup rsync blkid awk sed grep findmnt btrfs dracut file; do need "$c"; done

[[ -b "$DISK" ]] || { echo "Disk not found: $DISK"; exit 1; }
[[ -f "$IMAGE" ]] || { echo "Image not found: $IMAGE"; exit 1; }
[[ -f "$UEFI_DIR/RPI_EFI.fd" ]] || { echo "Missing $UEFI_DIR/RPI_EFI.fd (extract PFTF zip)"; exit 1; }

echo ">>> SAFETY CHECK: ABOUT TO ERASE $DISK"
lsblk -o NAME,SIZE,MODEL "$DISK"
echo "Ctrl+C NOW if wrong disk..."
sleep 5

TMPRAW=""
cleanup() {
  set +e
  umount -R /mnt/src 2>/dev/null || true
  umount -R /mnt/dst 2>/dev/null || true
  umount -R /mnt/dst/root/dev/pts 2>/dev/null || true
  [[ -n "${LOOP:-}" ]] && losetup -d "$LOOP" 2>/dev/null || true
  [[ -n "$TMPRAW" ]] && rm -f "$TMPRAW" 2>/dev/null || true
}
trap cleanup EXIT

# If .xz, decompress to temp
if [[ "$IMAGE" =~ \.xz$ ]]; then
  need xz
  TMPRAW="$(mktemp --suffix=.raw)"
  echo ">>> Decompressing $IMAGE -> $TMPRAW"
  xz -dc "$IMAGE" > "$TMPRAW"
  IMAGE="$TMPRAW"
fi

echo ">>> Unmounting anything using $DISK"
for mp in $(lsblk -lnpo MOUNTPOINT "$DISK" | awk 'NF'); do
  umount -R "$mp" 2>/dev/null || true
done

echo ">>> Wiping disk signatures"
wipefs -a "$DISK" || true

echo ">>> Creating GPT partition table (required for >2TiB disks)"
parted -s "$DISK" mklabel gpt

echo ">>> Creating partitions"
# p1: FAT32 ESP/BOOT
parted -s "$DISK" mkpart primary fat32 1MiB "$BOOT_SIZE"
parted -s "$DISK" set 1 esp on

# p2: ROOT ext4
parted -s "$DISK" mkpart primary ext4 "$BOOT_SIZE" "$ROOT_SIZE"

# p3: DATA ext4 (rest of disk)
parted -s "$DISK" mkpart primary ext4 "$ROOT_SIZE" 100%

parted -s "$DISK" print

BOOTDEV="${DISK}1"
ROOTDEV="${DISK}2"
DATADEV="${DISK}3"

echo ">>> Formatting"
mkfs.vfat -F32 -n EFI "$BOOTDEV"
mkfs.ext4 -F -L ROOT "$ROOTDEV"
mkfs.ext4 -F -L DATA "$DATADEV"

mkdir -p /mnt/src/{efi,boot,root,root_sub} /mnt/dst/{boot,root}

echo ">>> Loop-mounting image"
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
mount "$BOOTDEV" /mnt/dst/boot
mount "$ROOTDEV" /mnt/dst/root

echo ">>> Selecting Fedora btrfs root (prefer subvol=root)"
SRCROOT="/mnt/src/root"
if btrfs subvolume list /mnt/src/root | awk '{print $NF}' | grep -qx root; then
  mount -t btrfs -o subvol=root "$ROOTSRC" /mnt/src/root_sub
  SRCROOT="/mnt/src/root_sub"
  echo "Using subvol=root"
else
  echo "No subvol=root found; using top-level btrfs (may still work depending on image)"
fi

echo ">>> Copying Fedora root -> ext4 root (progress bar)"
rsync -aHAX --numeric-ids --info=progress2 "$SRCROOT"/ /mnt/dst/root/

echo ">>> Installing Fedora EFI files (EFI/*) onto FAT"
mkdir -p /mnt/dst/boot/EFI
rsync -a --delete --no-owner --no-group --no-perms /mnt/src/efi/EFI/ /mnt/dst/boot/EFI/

echo ">>> Installing Pi4 UEFI firmware LAST (PFTF) onto FAT"
# vfat can't chown; use no-owner/no-group/no-perms
rsync -a --delete --no-owner --no-group --no-perms "$UEFI_DIR"/ /mnt/dst/boot/

echo ">>> Writing Pi4 UEFI config.txt (important: dtoverlay=upstream-pi4)"
cat > /mnt/dst/boot/config.txt <<'EOF'
arm_64bit=1
enable_uart=1
dtoverlay=upstream-pi4
kernel=RPI_EFI.fd
EOF

echo ">>> Ensure /boot/efi mount exists in Fedora root + fstab"
mkdir -p /mnt/dst/root/boot/efi
BOOT_UUID="$(blkid -s UUID -o value "$BOOTDEV")"
grep -q "UUID=$BOOT_UUID .* /boot/efi " /mnt/dst/root/etc/fstab 2>/dev/null || \
  echo "UUID=$BOOT_UUID  /boot/efi  vfat  umask=0077,shortname=winnt  0  2" >> /mnt/dst/root/etc/fstab

DATA_UUID="$(blkid -s UUID -o value "$DATADEV")"
mkdir -p /mnt/dst/root/data
grep -q "UUID=$DATA_UUID .* /data " /mnt/dst/root/etc/fstab 2>/dev/null || \
  echo "UUID=$DATA_UUID  /data  ext4  defaults,noatime  0  2" >> /mnt/dst/root/etc/fstab

echo ">>> Bind mounts for dracut (and fix /var/tmp + devpts)"
mkdir -p /mnt/dst/root/{dev,proc,sys,run,tmp,var/tmp}
chmod 1777 /mnt/dst/root/tmp || true
chmod 1777 /mnt/dst/root/var/tmp || true

mount --bind /dev  /mnt/dst/root/dev
mount --bind /proc /mnt/dst/root/proc
mount --bind /sys  /mnt/dst/root/sys
mount --bind /run  /mnt/dst/root/run
mount --bind /tmp  /mnt/dst/root/tmp
mkdir -p /mnt/dst/root/dev/pts
mount -t devpts devpts /mnt/dst/root/dev/pts

echo ">>> Running dracut in chroot"
# dracut will regenerate initramfs for the installed kernel(s)
chroot /mnt/dst/root dracut -f --regenerate-all

echo ">>> Unmount chroot binds"
umount /mnt/dst/root/dev/pts || true
umount /mnt/dst/root/dev || true
umount /mnt/dst/root/proc || true
umount /mnt/dst/root/sys || true
umount /mnt/dst/root/run || true
umount /mnt/dst/root/tmp || true

echo ">>> Copy kernel + initramfs to FAT as kernel8.img/initramfs.img"
# Prefer kernel shipped in /usr/lib/modules/<ver>/vmlinuz (Fedora aarch64 images often do this)
VMLINUX="$(find /mnt/dst/root/usr/lib/modules -maxdepth 2 -type f -name vmlinuz | sort | tail -n1 || true)"
INITRD="$(ls -1 /mnt/dst/root/boot/initramfs-*.img 2>/dev/null | grep -v rescue | sort | tail -n1 || true)"

if [[ -z "$VMLINUX" || -z "$INITRD" ]]; then
  echo "ERROR: Could not locate vmlinuz or initramfs after dracut."
  echo "VMLINUX=$VMLINUX"
  echo "INITRD=$INITRD"
  exit 1
fi

cp -av "$VMLINUX" /mnt/dst/boot/kernel8.img
cp -av "$INITRD"  /mnt/dst/boot/initramfs.img

echo ">>> Sanity: kernel8.img must NOT be a PE/EFI binary"
KT="$(file -b /mnt/dst/boot/kernel8.img)"
echo "kernel8.img type: $KT"
if echo "$KT" | grep -qi 'PE32'; then
  echo "ERROR: kernel8.img is an EFI PE binary (wrong). You copied an EFI stub, not the Linux Image."
  echo "Stop here and tell Jones; we need to source the real Image/vmlinuz."
  exit 1
fi

echo ">>> Writing cmdline.txt for Fedora root"
ROOT_UUID="$(blkid -s UUID -o value "$ROOTDEV")"
cat > /mnt/dst/boot/cmdline.txt <<EOF
console=tty1 console=serial0,115200 root=UUID=$ROOT_UUID rootfstype=ext4 rw rootwait
EOF

echo ">>> Final sanity check"
ls -lah \
  /mnt/dst/boot/RPI_EFI.fd \
  /mnt/dst/boot/start4.elf \
  /mnt/dst/boot/fixup4.dat \
  /mnt/dst/boot/EFI/BOOT/BOOTAA64.EFI \
  /mnt/dst/boot/config.txt \
  /mnt/dst/boot/cmdline.txt \
  /mnt/dst/boot/kernel8.img \
  /mnt/dst/boot/initramfs.img

sync
umount -R /mnt/dst || true
umount -R /mnt/src || true
losetup -d "$LOOP" || true

echo "✅ Done. Boot Pi4 from MASH."
