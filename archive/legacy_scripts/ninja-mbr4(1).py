#!/usr/bin/env python3
"""
ninja-mbr4.py

Fedora RAW -> Pi4 USB disk, **MBR-first** with **exactly 4 partitions**.

Layout (MBR/msdos):
  p1 EFI   vfat   (default 1024MiB)  label EFI   flags: boot,lba
  p2 BOOT  ext4   (default 2048MiB)  label BOOT
  p3 ROOT  btrfs  (boot_end -> --root-end, default 1800GiB) label FEDORA
      subvols copied: root + (home if present) + (var if present)
  p4 DATA  ext4   (--root-end -> 100%) label DATA   mounted at /data

Source-of-truth base: holy-loop-fedora-ninja.py (V6), merged with the
MBR 4-part logic + grub stub patching from the other variants.

Usage:
  sudo ./ninja-mbr4.py Fedora-KDE-Desktop-Disk-43-1.6.aarch64.raw \
    --disk /dev/sda \
    --uefi-dir ./rpi4uefi

Optional:
  --efi-size 1024MiB
  --boot-size 2048MiB
  --root-end 1800GiB          # where partition 3 ends; partition 4 uses the rest
  --no-dracut                 # not recommended

Requirements:
  parted, wipefs, mkfs.vfat, mkfs.ext4, mkfs.btrfs, losetup, rsync, blkid, btrfs, dracut, findmnt, udevadm
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


def rsync_vfat_safe(src: Path, dst: Path, desc: str, delete=False):
    """
    VFAT cannot chown; do a safe copy with no owners/groups/perms.
    """
    banner(desc)
    cmd = ["rsync", "-rltD", "--no-owner", "--no-group", "--no-perms"]
    if delete:
        cmd.append("--delete")
    cmd += [f"{src}/", f"{dst}/"]
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
                    opts = re.sub(r"\brootflags=[^\s]+\b", "rootflags=subvol=root", opts)
                else:
                    opts = opts + " rootflags=subvol=root"

                out.append("options " + opts.strip() + "\n")
            else:
                out.append(line)

        f.write_text("".join(out), encoding="utf-8")
    print("✅ BLS patched.")


def write_grub_stub(efi_fedora_dir: Path, boot_uuid: str):
    """
    Ensure /EFI/fedora/grub.cfg exists on the EFI partition and points at /boot's UUID.
    This is the classic 'GRUB stub redirector' Fedora uses.
    """
    mkdirp(efi_fedora_dir)
    stub_path = efi_fedora_dir / "grub.cfg"
    stub_content = (
        f"search --no-floppy --fs-uuid --set=dev {boot_uuid}\n"
        "set prefix=($dev)/grub2\n"
        "configfile $prefix/grub.cfg\n"
    )
    write_file(stub_path, stub_content)


def main():
    if os.geteuid() != 0:
        die("Run as root: sudo ./ninja-mbr4.py ...")

    ap = argparse.ArgumentParser()
    ap.add_argument("image", help="Fedora *.raw image")
    ap.add_argument("--disk", default="/dev/sda", help="Target disk (MASH)")
    ap.add_argument("--uefi-dir", default="./rpi4uefi", help="PFTF UEFI dir (must contain RPI_EFI.fd)")
    ap.add_argument("--efi-size", default="1024MiB", help="EFI size (default 1024MiB)")
    ap.add_argument("--boot-size", default="2048MiB", help="/boot size (default 2048MiB)")
    ap.add_argument("--root-end", default="1800GiB", help="End of ROOT partition (p3). p4 (DATA) uses the rest.")
    ap.add_argument("--no-dracut", action="store_true", help="Skip dracut (not recommended)")
    args = ap.parse_args()

    image = Path(args.image).resolve()
    disk = args.disk
    uefi_dir = Path(args.uefi_dir).resolve()

    for c in ["parted", "wipefs", "mkfs.vfat", "mkfs.ext4", "mkfs.btrfs", "losetup", "rsync", "blkid", "btrfs", "dracut", "findmnt", "udevadm"]:
        need(c)

    if not image.exists():
        die(f"Image not found: {image}")
    if not Path(disk).exists():
        die(f"Disk not found: {disk}")
    if not (uefi_dir / "RPI_EFI.fd").exists():
        die(f"Missing {uefi_dir}/RPI_EFI.fd")

    SRC = Path("/mnt/ninja_src")
    DST = Path("/mnt/ninja_dst")

    loopdev = None

    def cleanup():
        nonlocal loopdev

        # Unmount (best-effort) in reverse order
        for p in [
            DST / "root_sub_root" / "boot" / "efi",
            DST / "efi",
            DST / "boot",
            DST / "root_sub_root" / "data",
            DST / "data",
            DST / "root_sub_root" / "var",
            DST / "root_sub_root" / "home",
            DST / "root_sub_root" / "dev" / "pts",
            DST / "root_sub_root" / "dev",
            DST / "root_sub_root" / "proc",
            DST / "root_sub_root" / "sys",
            DST / "root_sub_root" / "run",
            DST / "root_sub_root" / "tmp",
            DST / "root_sub_root",
            DST / "root_top",
            DST / "root_sub_home",
            DST / "root_sub_var",
            DST / "root_sub_root",
            SRC / "root_sub_root",
            SRC / "root_sub_home",
            SRC / "root_sub_var",
            SRC / "root_top",
            SRC / "boot",
            SRC / "efi",
        ]:
            umount(p)

        if loopdev:
            sh(["losetup", "-d", loopdev], check=False)
            loopdev = None

        udev_settle()

    try:
        banner("SAFETY CHECK: ABOUT TO ERASE TARGET DISK (MBR, 4 partitions)")
        lsblk_tree(disk)
        print(f"\nDisk: {disk} | Image: {image.name} | ROOT end: {args.root_end}")
        print("Ctrl+C now if that's not the right disk.\n")
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

        # ---- partition (MBR/msdos, 4 parts always) ----
        banner("Partitioning (MBR/msdos) — 4 partitions")
        efi_end_mib = parse_size_to_mib(args.efi_size)
        boot_size_mib = parse_size_to_mib(args.boot_size)
        boot_end_mib = efi_end_mib + boot_size_mib

        efi_start = "4MiB"
        efi_end = f"{efi_end_mib}MiB"
        boot_start = efi_end
        boot_end = f"{boot_end_mib}MiB"

        sh(["parted", "-s", disk, "mklabel", "msdos"])
        sh(["parted", "-s", "-a", "optimal", disk, "mkpart", "primary", "fat32", efi_start, efi_end])
        sh(["parted", "-s", disk, "set", "1", "boot", "on"])
        sh(["parted", "-s", disk, "set", "1", "lba", "on"])

        sh(["parted", "-s", "-a", "optimal", disk, "mkpart", "primary", "ext4", boot_start, boot_end])
        sh(["parted", "-s", "-a", "optimal", disk, "mkpart", "primary", "btrfs", boot_end, args.root_end])
        sh(["parted", "-s", "-a", "optimal", disk, "mkpart", "primary", "ext4", args.root_end, "100%"])

        sh(["parted", "-s", disk, "print"])
        udev_settle()

        efi_dev = f"{disk}1"
        boot_dev = f"{disk}2"
        root_dev = f"{disk}3"
        data_dev = f"{disk}4"

        # ---- format ----
        banner("Formatting filesystems")
        sh(["mkfs.vfat", "-F", "32", "-n", "EFI", efi_dev])
        sh(["mkfs.ext4", "-F", "-L", "BOOT", boot_dev])
        sh(["mkfs.btrfs", "-f", "-L", "FEDORA", root_dev])
        sh(["mkfs.ext4", "-F", "-L", "DATA", data_dev])
        udev_settle()

        # ---- loop mount image ----
        banner("Loop-mounting Fedora image")
        loopdev = sh(["losetup", "--show", "-Pf", str(image)], capture=True)
        sh(["lsblk", loopdev], check=False)

        img_efi = f"{loopdev}p1"
        img_boot = f"{loopdev}p2"
        img_root = f"{loopdev}p3"

        # ---- mount sources ----
        banner("Mounting image partitions")
        for sub in ["efi", "boot", "root_top", "root_sub_root", "root_sub_home", "root_sub_var"]:
            mkdirp(SRC / sub)

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
        for sub in ["efi", "boot", "data", "root_top", "root_sub_root", "root_sub_home", "root_sub_var"]:
            mkdirp(DST / sub)

        sh(["mount", efi_dev, str(DST / "efi")])
        sh(["mount", boot_dev, str(DST / "boot")])
        sh(["mount", data_dev, str(DST / "data")])
        sh(["mount", "-t", "btrfs", root_dev, str(DST / "root_top")])

        # create destination subvols
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

        # ---- copy btrfs subvols ----
        rsync_progress(SRC / "root_sub_root", DST / "root_sub_root", "Copying Fedora btrfs subvol: root")
        if has_home:
            rsync_progress(SRC / "root_sub_home", DST / "root_sub_home", "Copying Fedora btrfs subvol: home")
        if has_var:
            rsync_progress(SRC / "root_sub_var", DST / "root_sub_var", "Copying Fedora btrfs subvol: var")

        # ---- copy /boot partition ----
        rsync_progress(SRC / "boot", DST / "boot", "Copying Fedora /boot partition -> real /boot (ext4)")

        # ---- mount /boot + /boot/efi inside target root ----
        banner("Mounting /boot and /boot/efi inside target root")
        mkdirp(DST / "root_sub_root" / "boot")
        mkdirp(DST / "root_sub_root" / "boot" / "efi")
        sh(["mount", "--bind", str(DST / "boot"), str(DST / "root_sub_root" / "boot")])
        sh(["mount", "--bind", str(DST / "efi"), str(DST / "root_sub_root" / "boot" / "efi")])

        # ---- install Fedora EFI loaders onto EFI ----
        banner("Installing Fedora EFI loaders (EFI/*) onto EFI (FAT32)")
        mkdirp(DST / "efi" / "EFI")
        rsync_vfat_safe(SRC / "efi" / "EFI", DST / "efi" / "EFI", "Copy Fedora EFI tree to EFI partition", delete=False)

        # ---- install PFTF UEFI onto EFI (LAST) ----
        banner("Installing Pi4 UEFI (PFTF) onto EFI (LAST)")
        rsync_vfat_safe(uefi_dir, DST / "efi", "Copy PFTF UEFI firmware to EFI partition", delete=False)

        # ---- write config.txt (PFTF known-good) ----
        banner("Writing Pi4 UEFI config.txt")
        config_txt = """# Pi4 UEFI (PFTF) boot config for Fedora on USB (MBR, 4-part)
arm_64bit=1
enable_uart=1
enable_gic=1
armstub=RPI_EFI.fd
disable_commandline_tags=2

[pi4]
dtoverlay=upstream-pi4

[all]
"""
        write_file(DST / "efi" / "config.txt", config_txt)

        # ---- fstab + grub stub + BLS patch ----
        banner("Writing UUID-based /etc/fstab + GRUB stub + patching BLS")
        efi_uuid = blkid_uuid(efi_dev)
        boot_uuid = blkid_uuid(boot_dev)
        root_uuid = blkid_uuid(root_dev)
        data_uuid = blkid_uuid(data_dev)

        fstab_lines = [
            f"UUID={root_uuid}  /         btrfs  subvol=root,compress=zstd:1,defaults,noatime  0 0",
            f"UUID={root_uuid}  /home     btrfs  subvol=home,compress=zstd:1,defaults,noatime  0 0" if has_home else "",
            f"UUID={root_uuid}  /var      btrfs  subvol=var,compress=zstd:1,defaults,noatime   0 0" if has_var else "",
            f"UUID={boot_uuid}  /boot     ext4   defaults,noatime  0 2",
            f"UUID={efi_uuid}   /boot/efi vfat   umask=0077,shortname=winnt  0 2",
            f"UUID={data_uuid}  /data     ext4   defaults,noatime  0 2",
        ]
        target_fstab = DST / "root_sub_root" / "etc" / "fstab"
        mkdirp(target_fstab.parent)
        write_file(target_fstab, "\n".join([l for l in fstab_lines if l.strip()]) + "\n")

        # GRUB stub on EFI -> points at /boot UUID
        write_grub_stub(DST / "efi" / "EFI" / "fedora", boot_uuid)

        # Patch BLS entries (on /boot)
        patch_bls_entries(DST / "boot" / "loader" / "entries", root_uuid)

        # ---- dracut in chroot ----
        if not args.no_dracut:
            banner("Bind mounts for chroot + fixing /var/tmp + devpts")

            root_path = DST / "root_sub_root"

            # Ensure mountpoints exist in the target rootfs
            mkdirp(root_path / "boot" / "efi")
            mkdirp(root_path / "data")
            if has_var:
                mkdirp(root_path / "var")
            if has_home:
                mkdirp(root_path / "home")

            # Bind the other btrfs subvols into place (important for a sane chroot)
            if has_var:
                sh(["mount", "--bind", str(DST / "root_sub_var"), str(root_path / "var")])
            if has_home:
                sh(["mount", "--bind", str(DST / "root_sub_home"), str(root_path / "home")])

            # Bind /data for completeness (not strictly required for dracut)
            sh(["mount", "--bind", str(DST / "data"), str(root_path / "data")])

            # Standard chroot bind mounts
            for p in ["dev", "proc", "sys", "run", "tmp"]:
                mkdirp(root_path / p)
            mkdirp(root_path / "dev" / "pts")
            mkdirp(root_path / "var" / "tmp")

            sh(["chmod", "1777", str(root_path / "tmp")], check=False)
            sh(["chmod", "1777", str(root_path / "var" / "tmp")], check=False)

            sh(["mount", "--bind", "/dev", str(root_path / "dev")])
            sh(["mount", "-t", "devpts", "devpts", str(root_path / "dev" / "pts")], check=False)
            sh(["mount", "--bind", "/proc", str(root_path / "proc")])
            sh(["mount", "--bind", "/sys", str(root_path / "sys")])
            sh(["mount", "--bind", "/run", str(root_path / "run")], check=False)
            sh(["mount", "--bind", "/tmp", str(root_path / "tmp")], check=False)

            banner("Running dracut in chroot (regenerate all)")
            sh(["chroot", str(root_path), "dracut", "--regenerate-all", "--force"], check=False)

        # ---- final sanity ----
        banner("Final sanity checks")
        must = ["start4.elf", "fixup4.dat", "RPI_EFI.fd", "EFI/BOOT/BOOTAA64.EFI", "config.txt", "EFI/fedora/grub.cfg"]
        print("EFI must contain:")
        for p in must:
            full = DST / "efi" / p
            print(f"  {'✅' if full.exists() else '❌'} {full}")

        print("\nBoot partition must contain BLS:")
        print(f"  loader/entries exists: {'✅' if (DST / 'boot' / 'loader' / 'entries').exists() else '❌'}")

        print("\nfstab written at:")
        print(f"  {target_fstab}")

        banner("DONE")
        print("✅ Flash complete (MBR + 4 partitions).")
        print("Boot flow:")
        print("  Pi ROM -> start4.elf -> PFTF UEFI (RPI_EFI.fd) -> BOOTAA64.EFI (Fedora) -> GRUB -> kernel+initramfs from /boot -> btrfs subvol=root")
        print("\nIf it drops to GRUB again:")
        print("  - confirm EFI/EFI/fedora/grub.cfg exists and points at the BOOT UUID")
        print("  - press 'e' on the entry and confirm options include:")
        print(f"      root=UUID={root_uuid} rootflags=subvol=root")

    finally:
        cleanup()


if __name__ == "__main__":
    main()
