#!/usr/bin/env python3
"""
🎸 HOLY-LOOP-FEDORA-NINJA-V6: THE FINAL MASTERPIECE 🎷

- Fixes the GRUB stub UUID mismatch (no more grub> prompt!).
- Bridges Btrfs subvolumes for Dracut.
- Perfectly merges PFTF UEFI and Fedora loaders.
"""

import argparse
import os
import re
import shutil
import subprocess
import sys
import time
from pathlib import Path

# ---------- ⚡ NINJA HELPERS ⚡ ----------

def sh(cmd, check=True, capture=False):
    if isinstance(cmd, str):
        p = subprocess.run(cmd, shell=True, check=check,
                           stdout=subprocess.PIPE if capture else None,
                           stderr=subprocess.PIPE if capture else None,
                           text=True)
    else:
        p = subprocess.run(cmd, shell=False, check=check,
                           stdout=subprocess.PIPE if capture else None,
                           stderr=subprocess.PIPE if capture else None,
                           text=True)
    if capture:
        return (p.stdout or "").strip()
    return ""

def is_actually_mounted(path):
    return os.path.ismount(str(path))

def banner(title, icon="🚀"):
    width = 50
    print(f"\n\033[95m╭{'─' * width}╮\033[0m")
    print(f"\033[95m│\033[0m  {icon}  \033[1m{title.upper():<{width-8}}\033[0m \033[95m│\033[0m")
    print(f"\033[95m╰{'─' * width}╯\033[0m")

def rsync_progress(src, dst, desc):
    banner(desc, icon="🚚")
    cmd = ["rsync", "-aHAX", "--numeric-ids", "--info=progress2", f"{src}/", f"{dst}/"]
    subprocess.run(cmd, check=True)

def rsync_vfat_safe(src, dst, desc):
    banner(desc, icon="💾")
    cmd = ["rsync", "-rltD", "--no-owner", "--no-group", "--no-perms", f"{src}/", f"{dst}/"]
    sh(cmd, check=True)

# ---------- 🏗️ MAIN LOGIC 🏗️ ----------

def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("image", help="Fedora *.raw image")
    ap.add_argument("--disk", default="/dev/sda", help="Target disk")
    ap.add_argument("--uefi-dir", default="./rpi4uefi", help="PFTF UEFI dir")
    args = ap.parse_args()

    image, disk = Path(args.image).resolve(), args.disk
    uefi_dir = Path(args.uefi_dir).resolve()
    SRC, DST = Path("/mnt/ninja_src"), Path("/mnt/ninja_dst")
    loopdev = None

    def cleanup():
        nonlocal loopdev
        print("\n\033[93m🧹 Sweeping up the dojo...\033[0m")
        mounts = [
            DST/"root_sub_root/boot/efi", DST/"root_sub_root/boot",
            DST/"root_sub_root/var", DST/"root_sub_root/dev/pts",
            DST/"root_sub_root/dev", DST/"root_sub_root/proc",
            DST/"root_sub_root/sys", DST/"root_sub_root/run",
            DST/"root_sub_root", DST/"root_top", DST/"efi", DST/"boot",
            SRC/"root_sub_root", SRC/"root_sub_home", SRC/"root_sub_var",
            SRC/"root_top", SRC/"boot", SRC/"efi"
        ]
        for m in mounts: sh(f"umount -l {m} 2>/dev/null", check=False)
        if loopdev: sh(f"losetup -d {loopdev} 2>/dev/null", check=False)

    try:
        banner("Safety Check", icon="🚨")
        sh(f"lsblk {disk}")
        time.sleep(3)

        # 1. CLEAN & PARTITION
        banner("Wiping & Partitioning", icon="🏗️")
        sh(f"wipefs -a {disk}")
        sh(f"parted -s {disk} mklabel gpt")
        sh(f"parted -s -a optimal {disk} mkpart primary fat32 4MiB 1024MiB") # EFI
        sh(f"parted -s {disk} set 1 esp on")
        sh(f"parted -s -a optimal {disk} mkpart primary ext4 1024MiB 3072MiB") # BOOT
        sh(f"parted -s -a optimal {disk} mkpart primary btrfs 3072MiB 100%") # ROOT

        # 2. FORMAT
        banner("Formatting", icon="🛠️")
        sh(f"mkfs.vfat -F 32 -n EFI {disk}1")
        sh(f"mkfs.ext4 -F -L BOOT {disk}2")
        sh(f"mkfs.btrfs -f -L FEDORA {disk}3")

        # 3. MOUNT & CLONE
        loopdev = sh(f"losetup --show -Pf {image}", capture=True)
        for d in [SRC, DST]:
            for sub in ["efi", "boot", "root_top", "root_sub_root", "root_sub_home", "root_sub_var"]:
                (d/sub).mkdir(parents=True, exist_ok=True)

        sh(f"mount {loopdev}p1 {SRC}/efi")
        sh(f"mount {loopdev}p2 {SRC}/boot")
        sh(f"mount -t btrfs {loopdev}p3 {SRC}/root_top")
        sh(f"mount {disk}1 {DST}/efi")
        sh(f"mount {disk}2 {DST}/boot")
        sh(f"mount -t btrfs {disk}3 {DST}/root_top")

        for name in ["root", "home", "var"]:
            sh(f"mount -t btrfs -o subvol={name} {loopdev}p3 {SRC}/root_sub_{name}")
            sh(f"btrfs subvolume create {DST}/root_top/{name}")
            sh(f"mount -t btrfs -o subvol={name} {disk}3 {DST}/root_sub_{name}")
            rsync_progress(SRC/f"root_sub_{name}", DST/f"root_sub_{name}", f"Cloning {name}")

        rsync_progress(SRC/"boot", DST/"boot", "Cloning /boot")

        # 4. UEFI & EFI MERGE
        banner("Merging EFI Loaders & PFTF", icon="🥧")
        (DST/"efi/EFI").mkdir(parents=True, exist_ok=True)
        rsync_vfat_safe(SRC/"efi/EFI", DST/"efi/EFI", "Fedora EFI Tree")
        rsync_vfat_safe(uefi_dir, DST/"efi", "PFTF Firmware")
        (DST/"efi/config.txt").write_text("arm_64bit=1\nenable_uart=1\narmstub=RPI_EFI.fd\ndtoverlay=upstream-pi4\n")

        # 5. THE MAGIC FIX: GRUB STUB PATCH
        banner("Patching GRUB Stub UUID", icon="🩹")
        boot_uuid = sh(f"blkid -s UUID -o value {disk}2", capture=True)
        stub_path = DST/"efi/EFI/fedora/grub.cfg"
        stub_content = f"search --no-floppy --fs-uuid --set=dev {boot_uuid}\nset prefix=($dev)/grub2\nconfigfile $prefix/grub.cfg\n"
        stub_path.write_text(stub_content)
        print(f"✅ Stub pointed to /boot UUID: {boot_uuid}")

        # 6. FSTAB & CHROOT
        banner("Final Config & Dracut", icon="📝")
        root_uuid = sh(f"blkid -s UUID -o value {disk}3", capture=True)
        efi_uuid = sh(f"blkid -s UUID -o value {disk}1", capture=True)
        fstab = f"UUID={root_uuid} / btrfs subvol=root,compress=zstd:1 0 0\n"
        fstab += f"UUID={root_uuid} /home btrfs subvol=home,compress=zstd:1 0 0\n"
        fstab += f"UUID={root_uuid} /var btrfs subvol=var,compress=zstd:1 0 0\n"
        fstab += f"UUID={boot_uuid} /boot ext4 defaults 0 2\n"
        fstab += f"UUID={efi_uuid} /boot/efi vfat defaults 0 2\n"
        (DST/"root_sub_root/etc/fstab").write_text(fstab)

        root_path = DST/"root_sub_root"
        sh(f"mount --bind {DST}/root_sub_var {root_path}/var")
        sh(f"mount --bind {DST}/boot {root_path}/boot")
        sh(f"mount --bind {DST}/efi {root_path}/boot/efi")
        for p in ["dev", "proc", "sys", "run"]: sh(f"mount --bind /{p} {root_path}/{p}")
        sh(f"mkdir -p {root_path}/var/tmp && chmod 1777 {root_path}/var/tmp")

        sh(f"chroot {root_path} dracut --regenerate-all --force")
        banner("Mission Accomplished!", icon="🏁")

    except Exception as e: print(f"\033[91m⚠️ ERROR: {e}\033[0m")
    finally: cleanup()

if __name__ == "__main__": main()
