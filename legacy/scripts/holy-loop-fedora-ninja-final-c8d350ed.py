#!/usr/bin/env python3
"""
holy-loop-fedora-ninja-final.py

Flash Fedora aarch64 RAW image onto a Pi4 USB disk so it boots via Pi4 UEFI (PFTF).

What this script does (the “we learned this the hard way” version):
- Partitions disk as:
    p1 EFI  (FAT32)  1GiB  -> Pi firmware + PFTF UEFI + Fedora EFI loaders
    p2 BOOT (ext4)   2GiB  -> Fedora /boot (kernel+initramfs+BLS)
    p3 ROOT (btrfs)  rest  -> Fedora root filesystem (subvols root/home/var)
    p4 DATA (ext4)   opt   -> optional data partition (GPT recommended)
- Copies Fedora ROOT from image btrfs subvols: root + home + var (no “missing var/home” surprises)
- Copies Fedora /boot partition from image -> real /boot (so dracut + BLS are sane)
- Installs Pi4 UEFI firmware (PFTF) onto EFI (vfat-safe rsync: no chown/perms)
- Merges Fedora EFI loaders (EFI/BOOT/BOOTAA64.EFI etc.) onto EFI
- Writes a known-good config.txt for Pi4 UEFI (PFTF)
- Creates /boot/efi mountpoint and writes UUID-based /etc/fstab
- Patches BLS entries to point at the *new* root UUID and sets rootflags=subvol=root
- Runs dracut in chroot with /boot + /boot/efi mounted, plus fixed /var/tmp and devpts

Usage:
  sudo ./holy-loop-fedora-ninja-final.py Fedora-KDE-Desktop-Disk-43-1.6.aarch64.raw \
    --disk /dev/sda \
    --uefi-dir ./rpi4uefi \
    --scheme gpt \
    --make-data

Notes:
- GPT is strongly recommended for 4TB disks. MBR can hit the 2TB/4TB geometry limits depending on the device.
- Requires: parted, wipefs, mkfs.vfat, mkfs.ext4, mkfs.btrfs, losetup, rsync, blkid, btrfs, dracut, findmnt
"""

import argparse
import os
import re
import shutil
import subprocess
import sys
import time
from pathlib import Path


# ---------- helpers ----------
def sh(cmd, check=True, capture=False):
    """
    Run command. cmd can be list or string.
    """
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


def need(binname: str):
    if shutil.which(binname) is None:
        die(f"Missing required command: {binname}")


def die(msg: str, code: int = 1):
    print(f"\n[FATAL] {msg}\n")
    sys.exit(code)


def banner(title: str):
    line = "─" * (len(title) + 4)
    print(f"\n╭{line}╮")
    print(f"│  {title}  │")
    print(f"╰{line}╯")


def mkdirp(p: Path):
    p.mkdir(parents=True, exist_ok=True)


def umount(path: Path):
    sh(["umount", "-R", str(path)], check=False)


def udev_settle():
    sh(["udevadm", "settle"], check=False)


def lsblk_tree(disk: str):
    sh(["lsblk", "-o", "NAME,SIZE,TYPE,FSTYPE,MOUNTPOINTS,MODEL", disk], check=False)


def blkid_uuid(dev: str) -> str:
    return sh(["blkid", "-s", "UUID", "-o", "value", dev], capture=True)


def parse_size_to_mib(s: str) -> int:
    s = s.strip()
    m = re.match(r"^(\d+)\s*(MiB|GiB)$", s, re.I)
    if not m:
        die(f"Size must be like 1024MiB or 2GiB, got: {s}")
    v = int(m.group(1))
    unit = m.group(2).lower()
    return v if unit == "mib" else v * 1024


def rsync_progress(src: Path, dst: Path, desc: str, extra_args=None):
    """
    rsync with a simple progress bar using --info=progress2
    """
    extra_args = extra_args or []
    banner(desc)
    cmd = ["rsync", "-aHAX", "--numeric-ids", "--info=progress2"] + extra_args + [f"{src}/", f"{dst}/"]
    proc = subprocess.Popen(cmd, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True)
    barw = 28
    last = -1
    try:
        for line in proc.stdout:
            m = re.search(r"\s(\d{1,3})%\s", line)
            if m:
                pct = int(m.group(1))
                if pct != last:
                    last = pct
                    filled = int((pct / 100) * barw)
                    bar = "█" * filled + " " * (barw - filled)
                    sys.stdout.write(f"\r[{bar}] {pct:3d}%")
                    sys.stdout.flush()
        rc = proc.wait()
        if last >= 0:
            sys.stdout.write("\r" + " " * (barw + 10) + "\r")
        if rc != 0:
            die(f"{desc} failed (exit {rc})")
        print("✅ Done.")
    finally:
        try:
            proc.kill()
        except Exception:
            pass


def rsync_vfat_safe(src: Path, dst: Path, desc: str):
    """
    VFAT cannot chown; do a safe copy with no owners/groups/perms.
    """
    banner(desc)
    cmd = ["rsync", "-rltD", "--delete", "--no-owner", "--no-group", "--no-perms", f"{src}/", f"{dst}/"]
    sh(cmd, check=True)


def write_file(path: Path, content: str):
    path.write_text(content, encoding="utf-8")


def patch_bls_entries(boot_entries_dir: Path, root_uuid: str):
    """
    Patch BLS entries under /boot/loader/entries/*.conf:
      - set root=UUID=<new>
      - ensure rootflags=subvol=root
    """
    if not boot_entries_dir.exists():
        print(f"⚠️  No BLS entries dir found: {boot_entries_dir} (skipping BLS patch)")
        return

    files = sorted(boot_entries_dir.glob("*.conf"))
    if not files:
        print(f"⚠️  No BLS entry files in {boot_entries_dir} (skipping BLS patch)")
        return

    print(f"🩹 Patching BLS entries in {boot_entries_dir} ...")
    for f in files:
        txt = f.read_text(encoding="utf-8", errors="ignore").splitlines(True)

        out = []
        for line in txt:
            if line.startswith("options "):
                opts = line[len("options "):].strip()

                # replace root=...
                if re.search(r"\broot=UUID=[0-9a-fA-F-]+\b", opts):
                    opts = re.sub(r"\broot=UUID=[0-9a-fA-F-]+\b", f"root=UUID={root_uuid}", opts)
                elif re.search(r"\broot=[^\s]+\b", opts):
                    opts = re.sub(r"\broot=[^\s]+\b", f"root=UUID={root_uuid}", opts)
                else:
                    opts = f"root=UUID={root_uuid} " + opts

                # ensure rootflags=subvol=root
                if re.search(r"\brootflags=", opts):
                    # overwrite whatever is there
                    opts = re.sub(r"\brootflags=[^\s]+\b", "rootflags=subvol=root", opts)
                else:
                    opts = opts + " rootflags=subvol=root"

                out.append("options " + opts.strip() + "\n")
            else:
                out.append(line)

        f.write_text("".join(out), encoding="utf-8")
    print("✅ BLS patched.")


def main():
    if os.geteuid() != 0:
        die("Run as root: sudo ./holy-loop-fedora-ninja-final.py ...")

    ap = argparse.ArgumentParser()
    ap.add_argument("image", help="Fedora *.raw image")
    ap.add_argument("--disk", default="/dev/sda", help="Target disk (MASH)")
    ap.add_argument("--uefi-dir", default="./rpi4uefi", help="PFTF UEFI dir (must contain RPI_EFI.fd)")
    ap.add_argument("--scheme", choices=["gpt", "mbr"], default="gpt", help="Partition scheme (gpt recommended)")
    ap.add_argument("--make-data", action="store_true", help="Create /data partition (GPT recommended)")
    ap.add_argument("--efi-size", default="1024MiB", help="EFI size (default 1024MiB)")
    ap.add_argument("--boot-size", default="2048MiB", help="/boot size (default 2048MiB)")
    ap.add_argument("--mbr-root-end", default="1800GiB", help="MBR-only: end of ROOT partition (default 1800GiB)")
    ap.add_argument("--no-dracut", action="store_true", help="Skip dracut (not recommended)")
    args = ap.parse_args()

    image = Path(args.image).resolve()
    disk = args.disk
    uefi_dir = Path(args.uefi_dir).resolve()

    for c in ["parted", "wipefs", "mkfs.vfat", "mkfs.ext4", "mkfs.btrfs", "losetup", "rsync", "blkid", "btrfs", "dracut", "findmnt"]:
        need(c)

    if not image.exists():
        die(f"Image not found: {image}")
    if not Path(disk).exists():
        die(f"Disk not found: {disk}")
    if not (uefi_dir / "RPI_EFI.fd").exists():
        die(f"Missing {uefi_dir}/RPI_EFI.fd")

    # mount roots
    SRC = Path("/mnt/ninja_src")
    DST = Path("/mnt/ninja_dst")

    loopdev = None

    def cleanup():
        nonlocal loopdev
        # unmount in “reverse annoyances” order
        for p in [
            DST / "root" / "boot" / "efi",
            DST / "efi",
            DST / "boot",
            DST / "root",
            SRC / "root_top",
            SRC / "root_sub_root",
            SRC / "root_sub_home",
            SRC / "root_sub_var",
            SRC / "boot",
            SRC / "efi",
            SRC
        ]:
            umount(p)
        # chroot bind mounts
        for p in [
            DST / "root" / "dev" / "pts",
            DST / "root" / "dev",
            DST / "root" / "proc",
            DST / "root" / "sys",
            DST / "root" / "run",
            DST / "root" / "tmp"
        ]:
            umount(p)
        if loopdev:
            sh(["losetup", "-d", loopdev], check=False)
            loopdev = None
        udev_settle()

    try:
        banner("SAFETY CHECK: ABOUT TO ERASE TARGET DISK")
        lsblk_tree(disk)
        print(f"\nDisk: {disk} | Scheme: {args.scheme} | Data: {'yes' if args.make_data else 'no'} | Image: {image.name}")
        print("Ctrl+C now if that's not MASH.\n")
        time.sleep(5)

        # ---- unmount anything on disk ----
        banner("Unmounting anything using target disk")
        mps = sh(["lsblk", "-lnpo", "MOUNTPOINT", disk], capture=True)
        for mp in [x.strip() for x in mps.splitlines() if x.strip()]:
            sh(["umount", "-R", mp], check=False)
        cleanup()

        # ---- wipe signatures ----
        banner("Wiping signatures")
        sh(["wipefs", "-a", disk], check=False)
        udev_settle()

        # ---- partition ----
        banner(f"Partitioning ({args.scheme.upper()})")
        efi_end_mib = parse_size_to_mib(args.efi_size)
        boot_size_mib = parse_size_to_mib(args.boot_size)
        boot_end_mib = efi_end_mib + boot_size_mib

        efi_start = "4MiB"
        efi_end = f"{efi_end_mib}MiB"
        boot_start = efi_end
        boot_end = f"{boot_end_mib}MiB"

        if args.scheme == "gpt":
            sh(["parted", "-s", disk, "mklabel", "gpt"])
            sh(["parted", "-s", "-a", "optimal", disk, "mkpart", "primary", "fat32", efi_start, efi_end])
            sh(["parted", "-s", disk, "set", "1", "esp", "on"])
            sh(["parted", "-s", "-a", "optimal", disk, "mkpart", "primary", "ext4", boot_start, boot_end])
            if args.make_data:
                sh(["parted", "-s", "-a", "optimal", disk, "mkpart", "primary", "btrfs", boot_end, "70%"])
                sh(["parted", "-s", "-a", "optimal", disk, "mkpart", "primary", "ext4", "70%", "100%"])
            else:
                sh(["parted", "-s", "-a", "optimal", disk, "mkpart", "primary", "btrfs", boot_end, "100%"])
        else:
            print("⚠️  MBR selected. On many 4TB USB drives this can fail due to msdos limits.")
            sh(["parted", "-s", disk, "mklabel", "msdos"])
            sh(["parted", "-s", "-a", "optimal", disk, "mkpart", "primary", "fat32", efi_start, efi_end])
            sh(["parted", "-s", disk, "set", "1", "boot", "on"])
            sh(["parted", "-s", disk, "set", "1", "lba", "on"])
            sh(["parted", "-s", "-a", "optimal", disk, "mkpart", "primary", "ext4", boot_start, boot_end])
            sh(["parted", "-s", "-a", "optimal", disk, "mkpart", "primary", "btrfs", boot_end, args.mbr_root_end])
            if args.make_data:
                sh(["parted", "-s", "-a", "optimal", disk, "mkpart", "primary", "ext4", args.mbr_root_end, "100%"])

        sh(["parted", "-s", disk, "print"])
        udev_settle()

        efi_dev = f"{disk}1"
        boot_dev = f"{disk}2"
        root_dev = f"{disk}3"
        data_dev = f"{disk}4" if args.make_data else None

        # ---- format ----
        banner("Formatting filesystems")
        sh(["mkfs.vfat", "-F", "32", "-n", "EFI", efi_dev])
        sh(["mkfs.ext4", "-F", "-L", "BOOT", boot_dev])
        sh(["mkfs.btrfs", "-f", "-L", "FEDORA", root_dev])
        if data_dev:
            sh(["mkfs.ext4", "-F", "-L", "DATA", data_dev])

        udev_settle()

        # ---- loop mount image ----
        banner("Loop-mounting Fedora image")
        loopdev = sh(["losetup", "--show", "-Pf", str(image)], capture=True)
        sh(["lsblk", loopdev])

        img_efi = f"{loopdev}p1"
        img_boot = f"{loopdev}p2"
        img_root = f"{loopdev}p3"

        # ---- mount sources ----
        banner("Mounting image partitions")
        mkdirp(SRC / "efi")
        mkdirp(SRC / "boot")
        mkdirp(SRC / "root_top")
        mkdirp(SRC / "root_sub_root")
        mkdirp(SRC / "root_sub_home")
        mkdirp(SRC / "root_sub_var")

        sh(["mount", img_efi, str(SRC / "efi")])
        sh(["mount", img_boot, str(SRC / "boot")])
        sh(["mount", "-t", "btrfs", img_root, str(SRC / "root_top")])

        subvols = sh(["btrfs", "subvolume", "list", str(SRC / "root_top")], capture=True)
        has_root = re.search(r"\bpath\s+root$", subvols, re.M) is not None
        has_home = re.search(r"\bpath\s+home$", subvols, re.M) is not None
        has_var = re.search(r"\bpath\s+var$", subvols, re.M) is not None
        if not has_root:
            die("Image does not contain btrfs subvol 'root' (unexpected for Fedora RAW)")

        sh(["mount", "-t", "btrfs", "-o", "subvol=root", img_root, str(SRC / "root_sub_root")])
        if has_home:
            sh(["mount", "-t", "btrfs", "-o", "subvol=home", img_root, str(SRC / "root_sub_home")])
        if has_var:
            sh(["mount", "-t", "btrfs", "-o", "subvol=var", img_root, str(SRC / "root_sub_var")])

        # ---- mount destinations ----
        banner("Mounting destination partitions")
        mkdirp(DST / "efi")
        mkdirp(DST / "boot")
        mkdirp(DST / "root_top")
        mkdirp(DST / "root_sub_root")
        mkdirp(DST / "root_sub_home")
        mkdirp(DST / "root_sub_var")

        sh(["mount", efi_dev, str(DST / "efi")])
        sh(["mount", boot_dev, str(DST / "boot")])
        sh(["mount", "-t", "btrfs", root_dev, str(DST / "root_top")])

        # create destination subvols to match Fedora layout
        banner("Creating destination btrfs subvols")
        sh(["btrfs", "subvolume", "create", str(DST / "root_top" / "root")])
        if has_home:
            sh(["btrfs", "subvolume", "create", str(DST / "root_top" / "home")])
        if has_var:
            sh(["btrfs", "subvolume", "create", str(DST / "root_top" / "var")])

        # mount them for rsync targets
        sh(["mount", "-t", "btrfs", "-o", "subvol=root", root_dev, str(DST / "root_sub_root")])
        if has_home:
            sh(["mount", "-t", "btrfs", "-o", "subvol=home", root_dev, str(DST / "root_sub_home")])
        if has_var:
            sh(["mount", "-t", "btrfs", "-o", "subvol=var", root_dev, str(DST / "root_sub_var")])

        # ---- copy root subvols ----
        rsync_progress(SRC / "root_sub_root", DST / "root_sub_root", "Copying Fedora btrfs subvol: root")
        if has_home:
            rsync_progress(SRC / "root_sub_home", DST / "root_sub_home", "Copying Fedora btrfs subvol: home")
        if has_var:
            rsync_progress(SRC / "root_sub_var", DST / "root_sub_var", "Copying Fedora btrfs subvol: var")

        # ---- copy /boot partition ----
        rsync_progress(SRC / "boot", DST / "boot", "Copying Fedora /boot partition -> real /boot (ext4)")

        # ---- mount /boot and /boot/efi into the target root subvol ----
        banner("Mounting /boot and /boot/efi inside target root")
        mkdirp(DST / "root_sub_root" / "boot")
        mkdirp(DST / "root_sub_root" / "boot" / "efi")

        # bind /boot ext4 into root
        sh(["mount", "--bind", str(DST / "boot"), str(DST / "root_sub_root" / "boot")])
        # mount EFI into /boot/efi inside root
        sh(["mount", "--bind", str(DST / "efi"), str(DST / "root_sub_root" / "boot" / "efi")])

        # ---- install Fedora EFI loaders onto EFI ----
        banner("Installing Fedora EFI loaders (EFI/*) onto EFI (FAT32)")
        mkdirp(DST / "efi" / "EFI")
        rsync_vfat_safe(SRC / "efi" / "EFI", DST / "efi" / "EFI", "Copy Fedora EFI tree to EFI partition")

        # ---- install PFTF UEFI onto EFI (LAST) ----
        banner("Installing Pi4 UEFI (PFTF) onto EFI (LAST)")
        # copy PFTF directory contents onto the EFI partition
        rsync_vfat_safe(uefi_dir, DST / "efi", "Copy PFTF UEFI firmware to EFI partition")

        # ---- write config.txt (PFTF known-good) ----
        banner("Writing Pi4 UEFI config.txt")
        config_txt = """# Pi4 UEFI (PFTF) boot config for Fedora on USB
arm_64bit=1
enable_uart=1
enable_gic=1
armstub=RPI_EFI.fd
disable_commandline_tags=2

# Optional: if you need DT overlays, keep upstream-pi4.
# Fedora's own config uses upstream-pi4 for Pi 4 boards.
[pi4]
dtoverlay=upstream-pi4

[all]
"""
        write_file(DST / "efi" / "config.txt", config_txt)

        # ---- fstab ----
        banner("Writing UUID-based /etc/fstab")
        efi_uuid = blkid_uuid(efi_dev)
        boot_uuid = blkid_uuid(boot_dev)
        root_uuid = blkid_uuid(root_dev)

        fstab_lines = [
            f"UUID={root_uuid}  /         btrfs  subvol=root,compress=zstd:1,defaults,noatime  0 0",
            f"UUID={root_uuid}  /home     btrfs  subvol=home,compress=zstd:1,defaults,noatime  0 0" if has_home else "",
            f"UUID={root_uuid}  /var      btrfs  subvol=var,compress=zstd:1,defaults,noatime   0 0" if has_var else "",
            f"UUID={boot_uuid}  /boot     ext4   defaults,noatime  0 2",
            f"UUID={efi_uuid}   /boot/efi vfat   umask=0077,shortname=winnt  0 2",
        ]
        if data_dev:
            data_uuid = blkid_uuid(data_dev)
            fstab_lines.append(f"UUID={data_uuid}  /data     ext4   defaults,noatime  0 2")

        # write into target root
        target_fstab = DST / "root_sub_root" / "etc" / "fstab"
        mkdirp(target_fstab.parent)
        write_file(target_fstab, "\n".join([l for l in fstab_lines if l.strip()]) + "\n")

        # ---- patch BLS entries ----
        # BLS files live in /boot/loader/entries in Fedora (BLS-driven)
        bls_dir = DST / "boot" / "loader" / "entries"
        patch_bls_entries(bls_dir, root_uuid)

        # ---- dracut in chroot ----
        if not args.no_dracut:
            banner("Bind mounts for chroot + fixing /var/tmp + devpts")
            # Ensure required dirs exist inside target root
            mkdirp(DST / "root_sub_root" / "dev")
            mkdirp(DST / "root_sub_root" / "dev" / "pts")
            mkdirp(DST / "root_sub_root" / "proc")
            mkdirp(DST / "root_sub_root" / "sys")
            mkdirp(DST / "root_sub_root" / "run")
            mkdirp(DST / "root_sub_root" / "tmp")
            mkdirp(DST / "root_sub_root" / "var" / "tmp")

            # chmod sticky bits for tmp dirs
            sh(["chmod", "1777", str(DST / "root_sub_root" / "tmp")], check=False)
            sh(["chmod", "1777", str(DST / "root_sub_root" / "var" / "tmp")], check=False)

            # bind mounts
            sh(["mount", "--bind", "/dev", str(DST / "root_sub_root" / "dev")])
            sh(["mount", "-t", "devpts", "devpts", str(DST / "root_sub_root" / "dev" / "pts")], check=False)
            sh(["mount", "--bind", "/proc", str(DST / "root_sub_root" / "proc")])
            sh(["mount", "--bind", "/sys", str(DST / "root_sub_root" / "sys")])
            sh(["mount", "--bind", "/run", str(DST / "root_sub_root" / "run")], check=False)
            sh(["mount", "--bind", "/tmp", str(DST / "root_sub_root" / "tmp")], check=False)

            banner("Running dracut in chroot (regenerate all)")
            # IMPORTANT: /boot and /boot/efi are already bind-mounted above
            # Use --regenerate-all --force
            sh(["chroot", str(DST / "root_sub_root"), "dracut", "--regenerate-all", "--force"], check=False)

        # ---- final sanity ----
        banner("Final sanity checks")
        print("EFI must contain:")
        for p in ["start4.elf", "fixup4.dat", "RPI_EFI.fd", "EFI/BOOT/BOOTAA64.EFI", "config.txt"]:
            full = DST / "efi" / p
            print(f"  {'✅' if full.exists() else '❌'} {full}")

        print("\nBoot partition must contain BLS:")
        print(f"  loader/entries exists: {'✅' if (DST / 'boot' / 'loader' / 'entries').exists() else '❌'}")

        print("\nfstab written at:")
        print(f"  {target_fstab}")

        # ---- done ----
        banner("DONE")
        print("✅ Flash complete.")
        print("Next boot flow should be:")
        print("  Pi ROM -> start4.elf -> PFTF UEFI (RPI_EFI.fd) -> BOOTAA64.EFI (Fedora) -> GRUB -> kernel+initramfs from /boot -> btrfs subvol=root")
        print("\nIf you get stuck at GRUB again:")
        print("  - Press 'e' on the entry and confirm options include:")
        print(f"      root=UUID={root_uuid} rootflags=subvol=root")
        print("  - And confirm /etc/fstab has /boot and /boot/efi with correct UUIDs.\n")

    finally:
        cleanup()


if __name__ == "__main__":
    main()
