#!/usr/bin/env python3
"""
🎸 HOLY-LOOP-FEDORA-NINJA (MBR-ONLY, 4-PART EDITION) 🎷

Target layout (exactly 4 partitions, MBR/msdos):
  p1: EFI  - FAT32  512MiB  label EFI   flag: boot on
  p2: BOOT - ext4     2GiB  label BOOT
  p3: ROOT - btrfs   1.8TiB label ROOT  (subvols: root, home, var)
  p4: DATA - ext4    1.9TiB label DATA  (mounted at /data)
"""

import os
import subprocess
import time
from pathlib import Path

# ---------- 🔧 EDIT THESE (NO CLI ARGS) ----------
IMAGE    = "/home/drtweak/Fedora-KDE-Desktop-Disk-43-1.6.aarch64.raw"        # Source Fedora *.raw image (must have p1 EFI, p2 /boot, p3 btrfs)
DISK     = "/dev/sda"                   # Target disk (WIPED)
UEFI_DIR = "./rpi4uefi"                 # PFTF UEFI dir (copied into EFI partition)

# ---------- 🔧 SIZES (DO NOT ADD MORE PARTITIONS) ----------
EFI_SIZE_MIB  = 512
BOOT_SIZE_MIB = 2048
ROOT_SIZE_TIB = 1.8
DATA_SIZE_TIB = 1.9

# ---------- ⚡ NINJA HELPERS ⚡ ----------
def sh(cmd, check=True, capture=False):
    if isinstance(cmd, str):
        p = subprocess.run(
            cmd, shell=True, check=check,
            stdout=subprocess.PIPE if capture else None,
            stderr=subprocess.PIPE if capture else None,
            text=True
        )
    else:
        p = subprocess.run(
            cmd, shell=False, check=check,
            stdout=subprocess.PIPE if capture else None,
            stderr=subprocess.PIPE if capture else None,
            text=True
        )
    if capture:
        return (p.stdout or "").strip()
    return ""

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
    subprocess.run(cmd, check=True)

def tib_to_mib(tib: float) -> int:
    # 1 TiB = 1024 * 1024 MiB
    return int(round(tib * 1024 * 1024))

def main():
    image = Path(IMAGE).expanduser().resolve()
    disk = DISK
    uefi_dir = Path(UEFI_DIR).expanduser().resolve()

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
            DST/"root_sub_root/data",
            DST/"root_sub_root", DST/"root_top", DST/"data", DST/"efi", DST/"boot",
            SRC/"root_sub_root", SRC/"root_sub_home", SRC/"root_sub_var",
            SRC/"root_top", SRC/"boot", SRC/"efi"
        ]
        for m in mounts:
            sh(f"umount -l {m} 2>/dev/null", check=False)
        if loopdev:
            sh(f"losetup -d {loopdev} 2>/dev/null", check=False)

    try:
        banner("Safety Check", icon="🚨")
        if os.geteuid() != 0:
            raise RuntimeError("Run as root.")
        if not image.exists():
            raise RuntimeError(f"IMAGE not found: {image}")
        if not uefi_dir.exists():
            raise RuntimeError(f"UEFI_DIR not found: {uefi_dir}")

        sh(f"lsblk {disk}")
        time.sleep(2)

        # ---------- 1) CLEAN & PARTITION (MBR, 4 PARTS) ----------
        banner("Wiping & Partitioning (MBR, 4 parts)", icon="🏗️")
        sh(f"wipefs -a {disk}")
        sh(f"parted -s {disk} mklabel msdos")

        # Alignment start
        start_mib = 4

        # p1: EFI
        p1_end = start_mib + EFI_SIZE_MIB
        sh(f"parted -s {disk} unit MiB mkpart primary fat32 {start_mib} {p1_end}")
        sh(f"parted -s {disk} set 1 boot on")

        # p2: BOOT
        p2_end = p1_end + BOOT_SIZE_MIB
        sh(f"parted -s {disk} unit MiB mkpart primary ext4 {p1_end} {p2_end}")

        # p3: ROOT (Btrfs)
        root_mib_size = tib_to_mib(ROOT_SIZE_TIB)
        p3_end = p2_end + root_mib_size
        sh(f"parted -s {disk} unit MiB mkpart primary btrfs {p2_end} {p3_end}")

        # p4: DATA (Using 100% to avoid "outside the device" errors)
        sh(f"parted -s {disk} unit MiB mkpart primary ext4 {p3_end} 100%")

        sh(f"partprobe {disk}", check=False)
        time.sleep(2)

        # ---------- 2) FORMAT ----------
        banner("Formatting", icon="🛠️")
        sh(f"mkfs.vfat -F 32 -n EFI {disk}1")
        sh(f"mkfs.ext4 -F -L BOOT {disk}2")
        sh(f"mkfs.btrfs -f -L ROOT {disk}3")
        sh(f"mkfs.ext4 -F -L DATA {disk}4")

        # ---------- 3) MOUNT & CLONE ----------
        banner("Mount & Clone", icon="🧩")
        loopdev = sh(f"losetup --show -Pf {image}", capture=True)

        for d in [SRC, DST]:
            for sub in ["efi", "boot", "root_top", "root_sub_root", "root_sub_home", "root_sub_var", "data"]:
                (d/sub).mkdir(parents=True, exist_ok=True)

        # Source (image) mounts
        sh(f"mount {loopdev}p1 {SRC}/efi")
        sh(f"mount {loopdev}p2 {SRC}/boot")
        sh(f"mount -t btrfs {loopdev}p3 {SRC}/root_top")

        # Target mounts
        sh(f"mount {disk}1 {DST}/efi")
        sh(f"mount {disk}2 {DST}/boot")
        sh(f"mount -t btrfs {disk}3 {DST}/root_top")
        sh(f"mount {disk}4 {DST}/data")

        # Clone btrfs subvols
        for name in ["root", "home", "var"]:
            sh(f"mount -t btrfs -o subvol={name} {loopdev}p3 {SRC}/root_sub_{name}")
            sh(f"btrfs subvolume create {DST}/root_top/{name}")
            sh(f"mount -t btrfs -o subvol={name} {disk}3 {DST}/root_sub_{name}")
            rsync_progress(SRC/f"root_sub_{name}", DST/f"root_sub_{name}", f"Cloning {name}")

        # Clone /boot (ext4)
        rsync_progress(SRC/"boot", DST/"boot", "Cloning /boot")

        # ---------- 4) EFI MERGE (Fedora EFI + PFTF) ----------
        banner("Merging EFI Loaders & PFTF", icon="🥧")
        (DST/"efi/EFI").mkdir(parents=True, exist_ok=True)
        rsync_vfat_safe(SRC/"efi/EFI", DST/"efi/EFI", "Fedora EFI Tree")
        rsync_vfat_safe(uefi_dir, DST/"efi", "PFTF Firmware")

        # Minimal Pi config
        (DST/"efi/config.txt").write_text(
            "arm_64bit=1\n"
            "enable_uart=1\n"
            "armstub=RPI_EFI.fd\n"
            "dtoverlay=upstream-pi4\n"
        )

        # ---------- 5) GRUB STUB PATCH (points EFI stub to /boot UUID) ----------
        banner("Patching GRUB Stub UUID", icon="🩹")
        boot_uuid = sh(f"blkid -s UUID -o value {disk}2", capture=True)
        stub_path = DST/"efi/EFI/fedora/grub.cfg"
        stub_content = (
            f"search --no-floppy --fs-uuid --set=dev {boot_uuid}\n"
            "set prefix=($dev)/grub2\n"
            "configfile $prefix/grub.cfg\n"
        )
        (DST/"efi/EFI/fedora").mkdir(parents=True, exist_ok=True)
        stub_path.write_text(stub_content)

        # ---------- 6) FSTAB & CHROOT ----------
        banner("Final Config & Dracut", icon="📝")
        root_uuid = sh(f"blkid -s UUID -o value {disk}3", capture=True)
        efi_uuid  = sh(f"blkid -s UUID -o value {disk}1", capture=True)
        data_uuid = sh(f"blkid -s UUID -o value {disk}4", capture=True)

        fstab = ""
        fstab += f"UUID={root_uuid} / btrfs subvol=root,compress=zstd:1 0 0\n"
        fstab += f"UUID={root_uuid} /home btrfs subvol=home,compress=zstd:1 0 0\n"
        fstab += f"UUID={root_uuid} /var btrfs subvol=var,compress=zstd:1 0 0\n"
        fstab += f"UUID={boot_uuid} /boot ext4 defaults 0 2\n"
        fstab += f"UUID={efi_uuid} /boot/efi vfat defaults 0 2\n"
        fstab += f"UUID={data_uuid} /data ext4 defaults 0 2\n"

        (DST/"root_sub_root/etc").mkdir(parents=True, exist_ok=True)
        (DST/"root_sub_root/etc/fstab").write_text(fstab)

        # Ensure mountpoints exist in the target rootfs
        (DST/"root_sub_root/boot/efi").mkdir(parents=True, exist_ok=True)
        (DST/"root_sub_root/data").mkdir(parents=True, exist_ok=True)

        root_path = DST/"root_sub_root"
        sh(f"mount --bind {DST}/root_sub_var {root_path}/var")
        sh(f"mount --bind {DST}/boot {root_path}/boot")
        sh(f"mount --bind {DST}/efi {root_path}/boot/efi")
        sh(f"mount --bind {DST}/data {root_path}/data")
        for p in ["dev", "proc", "sys", "run"]:
            sh(f"mount --bind /{p} {root_path}/{p}")

        sh(f"mkdir -p {root_path}/var/tmp && chmod 1777 {root_path}/var/tmp")
        sh(f"chroot {root_path} dracut --regenerate-all --force")

        banner("Mission Accomplished!", icon="🏁")

    except Exception as e:
        print(f"\033[91m⚠️ ERROR: {e}\033[0m")
        raise
    finally:
        cleanup()

if __name__ == "__main__":
    main()
