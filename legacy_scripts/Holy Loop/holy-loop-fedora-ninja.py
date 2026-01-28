#!/usr/bin/env python3
"""
🎸 HOLY-LOOP-FEDORA-NINJA (ULTIMATE MBR EDITION) 🎷
- Combines 4TB MBR logic with BLS patching and Nuclear Unmounts.
- Fixes the 'stuck at GRUB menu' issue by updating kernel arguments.
"""

import os
import re
import subprocess
import time
from pathlib import Path

# ---------- 🔧 EDIT THESE ----------
IMAGE    = "Fedora-KDE-Desktop-Disk-43-1.6.aarch64.raw"
DISK     = "/dev/sda"
UEFI_DIR = "./rpi4uefi"

# ---------- ⚡ NINJA HELPERS ⚡ ----------
def sh(cmd, check=True, capture=False):
    shell = isinstance(cmd, str)
    p = subprocess.run(cmd, shell=shell, check=check,
                       stdout=subprocess.PIPE if capture else None,
                       stderr=subprocess.PIPE if capture else None, text=True)
    return p.stdout.strip() if capture else ""

def banner(title, icon="🚀"):
    print(f"\n\033[95m╭{'─' * 50}╮\n│  {icon}  {title.upper():<42} │\n╰{'─' * 50}╯\033[0m")

def rsync_progress(src, dst, desc):
    banner(desc, icon="🚚")
    subprocess.run(["rsync", "-aHAX", "--numeric-ids", "--info=progress2", f"{src}/", f"{dst}/"], check=True)

def patch_bls_entries(boot_entries_dir, root_uuid):
    """Rewrites kernel boot files to use the new drive's UUID."""
    if not boot_entries_dir.exists(): return
    print(f"🩹 Patching BLS entries in {boot_entries_dir}...")
    for f in boot_entries_dir.glob("*.conf"):
        content = f.read_text()
        # Update root UUID and ensure Btrfs flags are present
        content = re.sub(r"root=UUID=[0-9a-fA-F-]+", f"root=UUID={root_uuid}", content)
        if "rootflags=subvol=root" not in content:
            content = content.replace("options ", "options rootflags=subvol=root ")
        f.write_text(content)

def main():
    SRC, DST = Path("/mnt/ninja_src"), Path("/mnt/ninja_dst")
    img_path = Path(IMAGE).resolve()
    loopdev = None

    try:
        banner("Nuclear Unmount", icon="☢️")
        for _ in range(2):
            mps = sh(f"lsblk -lnpo MOUNTPOINT {DISK}", capture=True)
            for mp in sorted([x.strip() for x in mps.splitlines() if x.strip()], reverse=True):
                sh(f"umount -f -l {mp}", check=False)

        banner("Wiping & Partitioning", icon="🏗️")
        sh(f"wipefs -a {DISK}")
        sh(f"parted -s {DISK} mklabel msdos")
        sh(f"parted -s {DISK} unit MiB mkpart primary fat32 4 512")
        sh(f"parted -s {DISK} set 1 boot on")
        sh(f"parted -s {DISK} unit MiB mkpart primary ext4 512 2560")
        sh(f"parted -s {DISK} unit MiB mkpart primary btrfs 2560 1900000")
        sh(f"parted -s {DISK} unit MiB mkpart primary ext4 1900000 100%")

        banner("Formatting", icon="🛠️")
        sh(f"mkfs.vfat -F 32 -n EFI {DISK}1")
        sh(f"mkfs.ext4 -F -L BOOT {DISK}2")
        sh(f"mkfs.btrfs -f -L ROOT {DISK}3")
        sh(f"mkfs.ext4 -F -L DATA {DISK}4")

        loopdev = sh(f"losetup --show -Pf {img_path}", capture=True)
        for p in [SRC/"efi", SRC/"boot", SRC/"root_top", DST/"efi", DST/"boot", DST/"root_top"]: p.mkdir(parents=True, exist_ok=True)

        # Mounting
        sh(f"mount {DISK}1 {DST}/efi"); sh(f"mount {DISK}2 {DST}/boot"); sh(f"mount -t btrfs {DISK}3 {DST}/root_top")
        sh(f"mount {loopdev}p1 {SRC}/efi"); sh(f"mount {loopdev}p2 {SRC}/boot"); sh(f"mount -t btrfs {loopdev}p3 {SRC}/root_top")

        # Cloning Subvolumes
        for sub in ["root", "home", "var"]:
            sh(f"btrfs subvolume create {DST}/root_top/{sub}")
            target = DST/f"sub_{sub}"
            target.mkdir(exist_ok=True)
            sh(f"mount -t btrfs -o subvol={sub} {DISK}3 {target}")
            source = SRC/f"sub_{sub}"
            source.mkdir(exist_ok=True)
            sh(f"mount -t btrfs -o subvol={sub} {loopdev}p3 {source}")
            rsync_progress(source, target, f"Cloning {sub}")

        rsync_progress(SRC/"boot", DST/"boot", "Cloning /boot")

        banner("Merging EFI & PFTF", icon="🥧")
        sh(f"rsync -rltD --no-owner --no-group --no-perms {SRC}/efi/EFI/ {DST}/efi/EFI/")
        sh(f"rsync -rltD --no-owner --no-group --no-perms {UEFI_DIR}/ {DST}/efi/")

        banner("Fixing Boot Identities", icon="🩹")
        boot_uuid = sh(f"blkid -s UUID -o value {DISK}2", capture=True)
        root_uuid = sh(f"blkid -s UUID -o value {DISK}3", capture=True)

        # Patch GRUB Stub (the "redirector" file)
        stub = f"search --no-floppy --fs-uuid --set=dev {boot_uuid}\nset prefix=($dev)/grub2\nconfigfile $prefix/grub.cfg\n"
        (DST/"efi/EFI/fedora/grub.cfg").write_text(stub)

        # Patch BLS Entries (the kernel arguments)
        patch_bls_entries(DST/"boot/loader/entries", root_uuid)

        banner("Finalizing", icon="🏁")
        # Write FSTAB with new UUIDs
        fstab = f"UUID={root_uuid} / btrfs subvol=root,compress=zstd:1 0 0\nUUID={boot_uuid} /boot ext4 defaults 0 2\n"
        (DST/"sub_root/etc/fstab").write_text(fstab)

        print("✅ Mission Accomplished. Your 4TB drive is now a self-aware Fedora Ninja.")

    finally:
        # Cleanup logic
        sh(f"losetup -d {loopdev}", check=False)
        print("🧹 Dojo cleaned.")

if __name__ == "__main__": main()
