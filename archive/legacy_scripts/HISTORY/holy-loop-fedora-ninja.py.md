# History for holy-loop-fedora-ninja.py
Chosen master: **holy-loop-fedora-ninja-final.py** (sha c8d350edda08)

## Variants
- holy-loop-fedora-ninja-final.py (sha c8d350edda08)
- holy-loop-fedora-ninja-final.py (sha c8d350edda08)
- holy-loop-fedora-ninja-mbr4.py (sha cadf00ce07db)
- holy-loop-fedora-ninja-mbr4.py (sha 4f44bf66a0f5)
- holy-loop-fedora-ninja.py (sha dff77bbaff89)
- holy-loop-fedora-ninja.py (sha f3a162f90434)

## Diffs (against master)

### holy-loop-fedora-ninja-final.py → holy-loop-fedora-ninja-final.py

```diff

```

### holy-loop-fedora-ninja-mbr4.py → holy-loop-fedora-ninja-final.py

```diff
--- holy-loop-fedora-ninja-mbr4.py
+++ holy-loop-fedora-ninja-final.py
@@ -1,243 +1,537 @@
 #!/usr/bin/env python3
 """
-🎸 HOLY-LOOP-FEDORA-NINJA (MBR-ONLY, 4-PART EDITION) 🎷
-
-Target layout (exactly 4 partitions, MBR/msdos):
-  p1: EFI  - FAT32  512MiB  label EFI   flag: boot on
-  p2: BOOT - ext4     2GiB  label BOOT
-  p3: ROOT - btrfs   1.8TiB label ROOT  (subvols: root, home, var)
-  p4: DATA - ext4    1.9TiB label DATA  (mounted at /data)
+holy-loop-fedora-ninja-final.py
+
+Flash Fedora aarch64 RAW image onto a Pi4 USB disk so it boots via Pi4 UEFI (PFTF).
+
+What this script does (the “we learned this the hard way” version):
+- Partitions disk as:
+    p1 EFI  (FAT32)  1GiB  -> Pi firmware + PFTF UEFI + Fedora EFI loaders
+    p2 BOOT (ext4)   2GiB  -> Fedora /boot (kernel+initramfs+BLS)
+    p3 ROOT (btrfs)  rest  -> Fedora root filesystem (subvols root/home/var)
+    p4 DATA (ext4)   opt   -> optional data partition (GPT recommended)
+- Copies Fedora ROOT from image btrfs subvols: root + home + var (no “missing var/home” surprises)
+- Copies Fedora /boot partition from image -> real /boot (so dracut + BLS are sane)
+- Installs Pi4 UEFI firmware (PFTF) onto EFI (vfat-safe rsync: no chown/perms)
+- Merges Fedora EFI loaders (EFI/BOOT/BOOTAA64.EFI etc.) onto EFI
+- Writes a known-good config.txt for Pi4 UEFI (PFTF)
+- Creates /boot/efi mountpoint and writes UUID-based /etc/fstab
+- Patches BLS entries to point at the *new* root UUID and sets rootflags=subvol=root
+- Runs dracut in chroot with /boot + /boot/efi mounted, plus fixed /var/tmp and devpts
+
+Usage:
+  sudo ./holy-loop-fedora-ninja-final.py Fedora-KDE-Desktop-Disk-43-1.6.aarch64.raw \
+    --disk /dev/sda \
+    --uefi-dir ./rpi4uefi \
+    --scheme gpt \
+    --make-data
+
+Notes:
+- GPT is strongly recommended for 4TB disks. MBR can hit the 2TB/4TB geometry limits depending on the device.
+- Requires: parted, wipefs, mkfs.vfat, mkfs.ext4, mkfs.btrfs, losetup, rsync, blkid, btrfs, dracut, findmnt
 """

+import argparse
 import os
+import re
+import shutil
 import subprocess
+import sys
 import time
 from pathlib import Path

-# ---------- 🔧 EDIT THESE (NO CLI ARGS) ----------
-IMAGE    = "/path/to/Fedora.raw"        # Source Fedora *.raw image (must have p1 EFI, p2 /boot, p3 btrfs)
-DISK     = "/dev/sda"                   # Target disk (WIPED)
-UEFI_DIR = "./rpi4uefi"                 # PFTF UEFI dir (copied into EFI partition)
-
-# ---------- 🔧 SIZES (DO NOT ADD MORE PARTITIONS) ----------
-EFI_SIZE_MIB  = 512
-BOOT_SIZE_MIB = 2048
-ROOT_SIZE_TIB = 1.8
-DATA_SIZE_TIB = 1.9
-
-# ---------- ⚡ NINJA HELPERS ⚡ ----------
+
+# ---------- helpers ----------
 def sh(cmd, check=True, capture=False):
+    """
+    Run command. cmd can be list or string.
+    """
     if isinstance(cmd, str):
-        p = subprocess.run(
-            cmd, shell=True, check=check,
-            stdout=subprocess.PIPE if capture else None,
-            stderr=subprocess.PIPE if capture else None,
-            text=True
-        )
+        p = subprocess.run(cmd, shell=True, check=check,
+                           stdout=subprocess.PIPE if capture else None,
+                           stderr=subprocess.PIPE if capture else None,
+                           text=True)
     else:
-        p = subprocess.run(
-            cmd, shell=False, check=check,
-            stdout=subprocess.PIPE if capture else None,
-            stderr=subprocess.PIPE if capture else None,
-            text=True
-        )
+        p = subprocess.run(cmd, shell=False, check=check,
+                           stdout=subprocess.PIPE if capture else None,
+                           stderr=subprocess.PIPE if capture else None,
+                           text=True)
     if capture:
         return (p.stdout or "").strip()
     return ""

-def banner(title, icon="🚀"):
-    width = 50
-    print(f"\n\033[95m╭{'─' * width}╮\033[0m")
-    print(f"\033[95m│\033[0m  {icon}  \033[1m{title.upper():<{width-8}}\033[0m \033[95m│\033[0m")
-    print(f"\033[95m╰{'─' * width}╯\033[0m")
-
-def rsync_progress(src, dst, desc):
-    banner(desc, icon="🚚")
-    cmd = ["rsync", "-aHAX", "--numeric-ids", "--info=progress2", f"{src}/", f"{dst}/"]
-    subprocess.run(cmd, check=True)
-
-def rsync_vfat_safe(src, dst, desc):
-    banner(desc, icon="💾")
-    cmd = ["rsync", "-rltD", "--no-owner", "--no-group", "--no-perms", f"{src}/", f"{dst}/"]
-    subprocess.run(cmd, check=True)
-
-def tib_to_mib(tib: float) -> int:
-    # 1 TiB = 1024 * 1024 MiB
-    return int(round(tib * 1024 * 1024))
+
+def need(binname: str):
+    if shutil.which(binname) is None:
+        die(f"Missing required command: {binname}")
+
+
+def die(msg: str, code: int = 1):
+    print(f"\n[FATAL] {msg}\n")
+    sys.exit(code)
+
+
+def banner(title: str):
+    line = "─" * (len(title) + 4)
+    print(f"\n╭{line}╮")
+    print(f"│  {title}  │")
+    print(f"╰{line}╯")
+
+
+def mkdirp(p: Path):
+    p.mkdir(parents=True, exist_ok=True)
+
+
+def umount(path: Path):
+    sh(["umount", "-R", str(path)], check=False)
+
+
+def udev_settle():
+    sh(["udevadm", "settle"], check=False)
+
+
+def lsblk_tree(disk: str):
+    sh(["lsblk", "-o", "NAME,SIZE,TYPE,FSTYPE,MOUNTPOINTS,MODEL", disk], check=False)
+
+
+def blkid_uuid(dev: str) -> str:
+    return sh(["blkid", "-s", "UUID", "-o", "value", dev], capture=True)
+
+
+def parse_size_to_mib(s: str) -> int:
+    s = s.strip()
+    m = re.match(r"^(\d+)\s*(MiB|GiB)$", s, re.I)
+    if not m:
+        die(f"Size must be like 1024MiB or 2GiB, got: {s}")
+    v = int(m.group(1))
+    unit = m.group(2).lower()
+    return v if unit == "mib" else v * 1024
+
+
+def rsync_progress(src: Path, dst: Path, desc: str, extra_args=None):
+    """
+    rsync with a simple progress bar using --info=progress2
+    """
+    extra_args = extra_args or []
+    banner(desc)
+    cmd = ["rsync", "-aHAX", "--numeric-ids", "--info=progress2"] + extra_args + [f"{src}/", f"{dst}/"]
+    proc = subprocess.Popen(cmd, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True)
+    barw = 28
+    last = -1
+    try:
+        for line in proc.stdout:
+            m = re.search(r"\s(\d{1,3})%\s", line)
+            if m:
+                pct = int(m.group(1))
+                if pct != last:
+                    last = pct
+                    filled = int((pct / 100) * barw)
+                    bar = "█" * filled + " " * (barw - filled)
+                    sys.stdout.write(f"\r[{bar}] {pct:3d}%")
+                    sys.stdout.flush()
+        rc = proc.wait()
+        if last >= 0:
+            sys.stdout.write("\r" + " " * (barw + 10) + "\r")
+        if rc != 0:
+            die(f"{desc} failed (exit {rc})")
+        print("✅ Done.")
+    finally:
+        try:
+            proc.kill()
+        except Exception:
+            pass
+
+
+def rsync_vfat_safe(src: Path, dst: Path, desc: str):
+    """
+    VFAT cannot chown; do a safe copy with no owners/groups/perms.
+    """
+    banner(desc)
+    cmd = ["rsync", "-rltD", "--delete", "--no-owner", "--no-group", "--no-perms", f"{src}/", f"{dst}/"]
+    sh(cmd, check=True)
+
+
+def write_file(path: Path, content: str):
+    path.write_text(content, encoding="utf-8")
+
+
+def patch_bls_entries(boot_entries_dir: Path, root_uuid: str):
+    """
+    Patch BLS entries under /boot/loader/entries/*.conf:
+      - set root=UUID=<new>
+      - ensure rootflags=subvol=root
+    """
+    if not boot_entries_dir.exists():
+        print(f"⚠️  No BLS entries dir found: {boot_entries_dir} (skipping BLS patch)")
+        return
+
+    files = sorted(boot_entries_dir.glob("*.conf"))
+    if not files:
+        print(f"⚠️  No BLS entry files in {boot_entries_dir} (skipping BLS patch)")
+        return
+
+    print(f"🩹 Patching BLS entries in {boot_entries_dir} ...")
+    for f in files:
+        txt = f.read_text(encoding="utf-8", errors="ignore").splitlines(True)
+
+        out = []
+        for line in txt:
+            if line.startswith("options "):
+                opts = line[len("options "):].strip()
+
+                # replace root=...
+                if re.search(r"\broot=UUID=[0-9a-fA-F-]+\b", opts):
+                    opts = re.sub(r"\broot=UUID=[0-9a-fA-F-]+\b", f"root=UUID={root_uuid}", opts)
+                elif re.search(r"\broot=[^\s]+\b", opts):
+                    opts = re.sub(r"\broot=[^\s]+\b", f"root=UUID={root_uuid}", opts)
+                else:
+                    opts = f"root=UUID={root_uuid} " + opts
+
+                # ensure rootflags=subvol=root
+                if re.search(r"\brootflags=", opts):
+                    # overwrite whatever is there
+                    opts = re.sub(r"\brootflags=[^\s]+\b", "rootflags=subvol=root", opts)
+                else:
+                    opts = opts + " rootflags=subvol=root"
+
+                out.append("options " + opts.strip() + "\n")
+            else:
+                out.append(line)
+
+        f.write_text("".join(out), encoding="utf-8")
+    print("✅ BLS patched.")
+

 def main():
-    image = Path(IMAGE).expanduser().resolve()
-    disk = DISK
-    uefi_dir = Path(UEFI_DIR).expanduser().resolve()
-
-    SRC, DST = Path("/mnt/ninja_src"), Path("/mnt/ninja_dst")
+    if os.geteuid() != 0:
+        die("Run as root: sudo ./holy-loop-fedora-ninja-final.py ...")
+
+    ap = argparse.ArgumentParser()
+    ap.add_argument("image", help="Fedora *.raw image")
+    ap.add_argument("--disk", default="/dev/sda", help="Target disk (MASH)")
+    ap.add_argument("--uefi-dir", default="./rpi4uefi", help="PFTF UEFI dir (must contain RPI_EFI.fd)")
+    ap.add_argument("--scheme", choices=["gpt", "mbr"], default="gpt", help="Partition scheme (gpt recommended)")
+    ap.add_argument("--make-data", action="store_true", help="Create /data partition (GPT recommended)")
+    ap.add_argument("--efi-size", default="1024MiB", help="EFI size (default 1024MiB)")
+    ap.add_argument("--boot-size", default="2048MiB", help="/boot size (default 2048MiB)")
+    ap.add_argument("--mbr-root-end", default="1800GiB", help="MBR-only: end of ROOT partition (default 1800GiB)")
+    ap.add_argument("--no-dracut", action="store_true", help="Skip dracut (not recommended)")
+    args = ap.parse_args()
+
+    image = Path(args.image).resolve()
+    disk = args.disk
+    uefi_dir = Path(args.uefi_dir).resolve()
+
+    for c in ["parted", "wipefs", "mkfs.vfat", "mkfs.ext4", "mkfs.btrfs", "losetup", "rsync", "blkid", "btrfs", "dracut", "findmnt"]:
+        need(c)
+
+    if not image.exists():
+        die(f"Image not found: {image}")
+    if not Path(disk).exists():
+        die(f"Disk not found: {disk}")
+    if not (uefi_dir / "RPI_EFI.fd").exists():
+        die(f"Missing {uefi_dir}/RPI_EFI.fd")
+
+    # mount roots
+    SRC = Path("/mnt/ninja_src")
+    DST = Path("/mnt/ninja_dst")
+
     loopdev = None

     def cleanup():
         nonlocal loopdev
-        print("\n\033[93m🧹 Sweeping up the dojo...\033[0m")
-        mounts = [
-            DST/"root_sub_root/boot/efi", DST/"root_sub_root/boot",
-            DST/"root_sub_root/var", DST/"root_sub_root/dev/pts",
-            DST/"root_sub_root/dev", DST/"root_sub_root/proc",
-            DST/"root_sub_root/sys", DST/"root_sub_root/run",
-            DST/"root_sub_root/data",
-            DST/"root_sub_root", DST/"root_top", DST/"data", DST/"efi", DST/"boot",
-            SRC/"root_sub_root", SRC/"root_sub_home", SRC/"root_sub_var",
-            SRC/"root_top", SRC/"boot", SRC/"efi"
+        # unmount in “reverse annoyances” order
+        for p in [
+            DST / "root" / "boot" / "efi",
+            DST / "efi",
+            DST / "boot",
+            DST / "root",
+            SRC / "root_top",
+            SRC / "root_sub_root",
+            SRC / "root_sub_home",
+            SRC / "root_sub_var",
+            SRC / "boot",
+            SRC / "efi",
+            SRC
+        ]:
+            umount(p)
+        # chroot bind mounts
+        for p in [
+            DST / "root" / "dev" / "pts",
+            DST / "root" / "dev",
+            DST / "root" / "proc",
+            DST / "root" / "sys",
+            DST / "root" / "run",
+            DST / "root" / "tmp"
+        ]:
+            umount(p)
+        if loopdev:
+            sh(["losetup", "-d", loopdev], check=False)
+            loopdev = None
+        udev_settle()
+
+    try:
+        banner("SAFETY CHECK: ABOUT TO ERASE TARGET DISK")
+        lsblk_tree(disk)
+        print(f"\nDisk: {disk} | Scheme: {args.scheme} | Data: {'yes' if args.make_data else 'no'} | Image: {image.name}")
+        print("Ctrl+C now if that's not MASH.\n")
+        time.sleep(5)
+
+        # ---- unmount anything on disk ----
+        banner("Unmounting anything using target disk")
+        mps = sh(["lsblk", "-lnpo", "MOUNTPOINT", disk], capture=True)
+        for mp in [x.strip() for x in mps.splitlines() if x.strip()]:
+            sh(["umount", "-R", mp], check=False)
+        cleanup()
+
+        # ---- wipe signatures ----
+        banner("Wiping signatures")
+        sh(["wipefs", "-a", disk], check=False)
+        udev_settle()
+
+        # ---- partition ----
+        banner(f"Partitioning ({args.scheme.upper()})")
+        efi_end_mib = parse_size_to_mib(args.efi_size)
+        boot_size_mib = parse_size_to_mib(args.boot_size)
+        boot_end_mib = efi_end_mib + boot_size_mib
+
+        efi_start = "4MiB"
+        efi_end = f"{efi_end_mib}MiB"
+        boot_start = efi_end
+        boot_end = f"{boot_end_mib}MiB"
+
+        if args.scheme == "gpt":
+            sh(["parted", "-s", disk, "mklabel", "gpt"])
+            sh(["parted", "-s", "-a", "optimal", disk, "mkpart", "primary", "fat32", efi_start, efi_end])
+            sh(["parted", "-s", disk, "set", "1", "esp", "on"])
+            sh(["parted", "-s", "-a", "optimal", disk, "mkpart", "primary", "ext4", boot_start, boot_end])
+            if args.make_data:
+                sh(["parted", "-s", "-a", "optimal", disk, "mkpart", "primary", "btrfs", boot_end, "70%"])
+                sh(["parted", "-s", "-a", "optimal", disk, "mkpart", "primary", "ext4", "70%", "100%"])
+            else:
+                sh(["parted", "-s", "-a", "optimal", disk, "mkpart", "primary", "btrfs", boot_end, "100%"])
+        else:
+            print("⚠️  MBR selected. On many 4TB USB drives this can fail due to msdos limits.")
+            sh(["parted", "-s", disk, "mklabel", "msdos"])
+            sh(["parted", "-s", "-a", "optimal", disk, "mkpart", "primary", "fat32", efi_start, efi_end])
+            sh(["parted", "-s", disk, "set", "1", "boot", "on"])
+            sh(["parted", "-s", disk, "set", "1", "lba", "on"])
+            sh(["parted", "-s", "-a", "optimal", disk, "mkpart", "primary", "ext4", boot_start, boot_end])
+            sh(["parted", "-s", "-a", "optimal", disk, "mkpart", "primary", "btrfs", boot_end, args.mbr_root_end])
+            if args.make_data:
+                sh(["parted", "-s", "-a", "optimal", disk, "mkpart", "primary", "ext4", args.mbr_root_end, "100%"])
+
+        sh(["parted", "-s", disk, "print"])
+        udev_settle()
+
+        efi_dev = f"{disk}1"
+        boot_dev = f"{disk}2"
+        root_dev = f"{disk}3"
+        data_dev = f"{disk}4" if args.make_data else None
+
+        # ---- format ----
... (diff truncated) ...
```

### holy-loop-fedora-ninja-mbr4.py → holy-loop-fedora-ninja-final.py

```diff
--- holy-loop-fedora-ninja-mbr4.py
+++ holy-loop-fedora-ninja-final.py
@@ -1,237 +1,537 @@
 #!/usr/bin/env python3
 """
-🎸 HOLY-LOOP-FEDORA-NINJA (MBR-ONLY, 4-PART EDITION) 🎷
-
-Target layout (exactly 4 partitions, MBR/msdos):
-  p1: EFI  - FAT32  512MiB  label EFI   flag: boot on
-  p2: BOOT - ext4     2GiB  label BOOT
-  p3: ROOT - btrfs   1.8TiB label ROOT  (subvols: root, home, var)
-  p4: DATA - ext4    1.9TiB label DATA  (mounted at /data)
+holy-loop-fedora-ninja-final.py
+
+Flash Fedora aarch64 RAW image onto a Pi4 USB disk so it boots via Pi4 UEFI (PFTF).
+
+What this script does (the “we learned this the hard way” version):
+- Partitions disk as:
+    p1 EFI  (FAT32)  1GiB  -> Pi firmware + PFTF UEFI + Fedora EFI loaders
+    p2 BOOT (ext4)   2GiB  -> Fedora /boot (kernel+initramfs+BLS)
+    p3 ROOT (btrfs)  rest  -> Fedora root filesystem (subvols root/home/var)
+    p4 DATA (ext4)   opt   -> optional data partition (GPT recommended)
+- Copies Fedora ROOT from image btrfs subvols: root + home + var (no “missing var/home” surprises)
+- Copies Fedora /boot partition from image -> real /boot (so dracut + BLS are sane)
+- Installs Pi4 UEFI firmware (PFTF) onto EFI (vfat-safe rsync: no chown/perms)
+- Merges Fedora EFI loaders (EFI/BOOT/BOOTAA64.EFI etc.) onto EFI
+- Writes a known-good config.txt for Pi4 UEFI (PFTF)
+- Creates /boot/efi mountpoint and writes UUID-based /etc/fstab
+- Patches BLS entries to point at the *new* root UUID and sets rootflags=subvol=root
+- Runs dracut in chroot with /boot + /boot/efi mounted, plus fixed /var/tmp and devpts
+
+Usage:
+  sudo ./holy-loop-fedora-ninja-final.py Fedora-KDE-Desktop-Disk-43-1.6.aarch64.raw \
+    --disk /dev/sda \
+    --uefi-dir ./rpi4uefi \
+    --scheme gpt \
+    --make-data
+
+Notes:
+- GPT is strongly recommended for 4TB disks. MBR can hit the 2TB/4TB geometry limits depending on the device.
+- Requires: parted, wipefs, mkfs.vfat, mkfs.ext4, mkfs.btrfs, losetup, rsync, blkid, btrfs, dracut, findmnt
 """

+import argparse
 import os
+import re
+import shutil
 import subprocess
+import sys
 import time
 from pathlib import Path

-# ---------- 🔧 EDIT THESE (NO CLI ARGS) ----------
-IMAGE    = "/home/drtweak/Fedora-KDE-Desktop-Disk-43-1.6.aarch64.raw"        # Source Fedora *.raw image (must have p1 EFI, p2 /boot, p3 btrfs)
-DISK     = "/dev/sda"                   # Target disk (WIPED)
-UEFI_DIR = "./rpi4uefi"                 # PFTF UEFI dir (copied into EFI partition)
-
-# ---------- 🔧 SIZES (DO NOT ADD MORE PARTITIONS) ----------
-EFI_SIZE_MIB  = 512
-BOOT_SIZE_MIB = 2048
-ROOT_SIZE_TIB = 1.8
-DATA_SIZE_TIB = 1.9
-
-# ---------- ⚡ NINJA HELPERS ⚡ ----------
+
+# ---------- helpers ----------
 def sh(cmd, check=True, capture=False):
+    """
+    Run command. cmd can be list or string.
+    """
     if isinstance(cmd, str):
-        p = subprocess.run(
-            cmd, shell=True, check=check,
-            stdout=subprocess.PIPE if capture else None,
-            stderr=subprocess.PIPE if capture else None,
-            text=True
-        )
+        p = subprocess.run(cmd, shell=True, check=check,
+                           stdout=subprocess.PIPE if capture else None,
+                           stderr=subprocess.PIPE if capture else None,
+                           text=True)
     else:
-        p = subprocess.run(
-            cmd, shell=False, check=check,
-            stdout=subprocess.PIPE if capture else None,
-            stderr=subprocess.PIPE if capture else None,
-            text=True
-        )
+        p = subprocess.run(cmd, shell=False, check=check,
+                           stdout=subprocess.PIPE if capture else None,
+                           stderr=subprocess.PIPE if capture else None,
+                           text=True)
     if capture:
         return (p.stdout or "").strip()
     return ""

-def banner(title, icon="🚀"):
-    width = 50
-    print(f"\n\033[95m╭{'─' * width}╮\033[0m")
-    print(f"\033[95m│\033[0m  {icon}  \033[1m{title.upper():<{width-8}}\033[0m \033[95m│\033[0m")
-    print(f"\033[95m╰{'─' * width}╯\033[0m")
-
-def rsync_progress(src, dst, desc):
-    banner(desc, icon="🚚")
-    cmd = ["rsync", "-aHAX", "--numeric-ids", "--info=progress2", f"{src}/", f"{dst}/"]
-    subprocess.run(cmd, check=True)
-
-def rsync_vfat_safe(src, dst, desc):
-    banner(desc, icon="💾")
-    cmd = ["rsync", "-rltD", "--no-owner", "--no-group", "--no-perms", f"{src}/", f"{dst}/"]
-    subprocess.run(cmd, check=True)
-
-def tib_to_mib(tib: float) -> int:
-    # 1 TiB = 1024 * 1024 MiB
-    return int(round(tib * 1024 * 1024))
+
+def need(binname: str):
+    if shutil.which(binname) is None:
+        die(f"Missing required command: {binname}")
+
+
+def die(msg: str, code: int = 1):
+    print(f"\n[FATAL] {msg}\n")
+    sys.exit(code)
+
+
+def banner(title: str):
+    line = "─" * (len(title) + 4)
+    print(f"\n╭{line}╮")
+    print(f"│  {title}  │")
+    print(f"╰{line}╯")
+
+
+def mkdirp(p: Path):
+    p.mkdir(parents=True, exist_ok=True)
+
+
+def umount(path: Path):
+    sh(["umount", "-R", str(path)], check=False)
+
+
+def udev_settle():
+    sh(["udevadm", "settle"], check=False)
+
+
+def lsblk_tree(disk: str):
+    sh(["lsblk", "-o", "NAME,SIZE,TYPE,FSTYPE,MOUNTPOINTS,MODEL", disk], check=False)
+
+
+def blkid_uuid(dev: str) -> str:
+    return sh(["blkid", "-s", "UUID", "-o", "value", dev], capture=True)
+
+
+def parse_size_to_mib(s: str) -> int:
+    s = s.strip()
+    m = re.match(r"^(\d+)\s*(MiB|GiB)$", s, re.I)
+    if not m:
+        die(f"Size must be like 1024MiB or 2GiB, got: {s}")
+    v = int(m.group(1))
+    unit = m.group(2).lower()
+    return v if unit == "mib" else v * 1024
+
+
+def rsync_progress(src: Path, dst: Path, desc: str, extra_args=None):
+    """
+    rsync with a simple progress bar using --info=progress2
+    """
+    extra_args = extra_args or []
+    banner(desc)
+    cmd = ["rsync", "-aHAX", "--numeric-ids", "--info=progress2"] + extra_args + [f"{src}/", f"{dst}/"]
+    proc = subprocess.Popen(cmd, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True)
+    barw = 28
+    last = -1
+    try:
+        for line in proc.stdout:
+            m = re.search(r"\s(\d{1,3})%\s", line)
+            if m:
+                pct = int(m.group(1))
+                if pct != last:
+                    last = pct
+                    filled = int((pct / 100) * barw)
+                    bar = "█" * filled + " " * (barw - filled)
+                    sys.stdout.write(f"\r[{bar}] {pct:3d}%")
+                    sys.stdout.flush()
+        rc = proc.wait()
+        if last >= 0:
+            sys.stdout.write("\r" + " " * (barw + 10) + "\r")
+        if rc != 0:
+            die(f"{desc} failed (exit {rc})")
+        print("✅ Done.")
+    finally:
+        try:
+            proc.kill()
+        except Exception:
+            pass
+
+
+def rsync_vfat_safe(src: Path, dst: Path, desc: str):
+    """
+    VFAT cannot chown; do a safe copy with no owners/groups/perms.
+    """
+    banner(desc)
+    cmd = ["rsync", "-rltD", "--delete", "--no-owner", "--no-group", "--no-perms", f"{src}/", f"{dst}/"]
+    sh(cmd, check=True)
+
+
+def write_file(path: Path, content: str):
+    path.write_text(content, encoding="utf-8")
+
+
+def patch_bls_entries(boot_entries_dir: Path, root_uuid: str):
+    """
+    Patch BLS entries under /boot/loader/entries/*.conf:
+      - set root=UUID=<new>
+      - ensure rootflags=subvol=root
+    """
+    if not boot_entries_dir.exists():
+        print(f"⚠️  No BLS entries dir found: {boot_entries_dir} (skipping BLS patch)")
+        return
+
+    files = sorted(boot_entries_dir.glob("*.conf"))
+    if not files:
+        print(f"⚠️  No BLS entry files in {boot_entries_dir} (skipping BLS patch)")
+        return
+
+    print(f"🩹 Patching BLS entries in {boot_entries_dir} ...")
+    for f in files:
+        txt = f.read_text(encoding="utf-8", errors="ignore").splitlines(True)
+
+        out = []
+        for line in txt:
+            if line.startswith("options "):
+                opts = line[len("options "):].strip()
+
+                # replace root=...
+                if re.search(r"\broot=UUID=[0-9a-fA-F-]+\b", opts):
+                    opts = re.sub(r"\broot=UUID=[0-9a-fA-F-]+\b", f"root=UUID={root_uuid}", opts)
+                elif re.search(r"\broot=[^\s]+\b", opts):
+                    opts = re.sub(r"\broot=[^\s]+\b", f"root=UUID={root_uuid}", opts)
+                else:
+                    opts = f"root=UUID={root_uuid} " + opts
+
+                # ensure rootflags=subvol=root
+                if re.search(r"\brootflags=", opts):
+                    # overwrite whatever is there
+                    opts = re.sub(r"\brootflags=[^\s]+\b", "rootflags=subvol=root", opts)
+                else:
+                    opts = opts + " rootflags=subvol=root"
+
+                out.append("options " + opts.strip() + "\n")
+            else:
+                out.append(line)
+
+        f.write_text("".join(out), encoding="utf-8")
+    print("✅ BLS patched.")
+

 def main():
-    image = Path(IMAGE).expanduser().resolve()
-    disk = DISK
-    uefi_dir = Path(UEFI_DIR).expanduser().resolve()
-
-    SRC, DST = Path("/mnt/ninja_src"), Path("/mnt/ninja_dst")
+    if os.geteuid() != 0:
+        die("Run as root: sudo ./holy-loop-fedora-ninja-final.py ...")
+
+    ap = argparse.ArgumentParser()
+    ap.add_argument("image", help="Fedora *.raw image")
+    ap.add_argument("--disk", default="/dev/sda", help="Target disk (MASH)")
+    ap.add_argument("--uefi-dir", default="./rpi4uefi", help="PFTF UEFI dir (must contain RPI_EFI.fd)")
+    ap.add_argument("--scheme", choices=["gpt", "mbr"], default="gpt", help="Partition scheme (gpt recommended)")
+    ap.add_argument("--make-data", action="store_true", help="Create /data partition (GPT recommended)")
+    ap.add_argument("--efi-size", default="1024MiB", help="EFI size (default 1024MiB)")
+    ap.add_argument("--boot-size", default="2048MiB", help="/boot size (default 2048MiB)")
+    ap.add_argument("--mbr-root-end", default="1800GiB", help="MBR-only: end of ROOT partition (default 1800GiB)")
+    ap.add_argument("--no-dracut", action="store_true", help="Skip dracut (not recommended)")
+    args = ap.parse_args()
+
+    image = Path(args.image).resolve()
+    disk = args.disk
+    uefi_dir = Path(args.uefi_dir).resolve()
+
+    for c in ["parted", "wipefs", "mkfs.vfat", "mkfs.ext4", "mkfs.btrfs", "losetup", "rsync", "blkid", "btrfs", "dracut", "findmnt"]:
+        need(c)
+
+    if not image.exists():
+        die(f"Image not found: {image}")
+    if not Path(disk).exists():
+        die(f"Disk not found: {disk}")
+    if not (uefi_dir / "RPI_EFI.fd").exists():
+        die(f"Missing {uefi_dir}/RPI_EFI.fd")
+
+    # mount roots
+    SRC = Path("/mnt/ninja_src")
+    DST = Path("/mnt/ninja_dst")
+
     loopdev = None

     def cleanup():
         nonlocal loopdev
-        print("\n\033[93m🧹 Sweeping up the dojo...\033[0m")
-        mounts = [
-            DST/"root_sub_root/boot/efi", DST/"root_sub_root/boot",
-            DST/"root_sub_root/var", DST/"root_sub_root/dev/pts",
-            DST/"root_sub_root/dev", DST/"root_sub_root/proc",
-            DST/"root_sub_root/sys", DST/"root_sub_root/run",
-            DST/"root_sub_root/data",
-            DST/"root_sub_root", DST/"root_top", DST/"data", DST/"efi", DST/"boot",
-            SRC/"root_sub_root", SRC/"root_sub_home", SRC/"root_sub_var",
-            SRC/"root_top", SRC/"boot", SRC/"efi"
+        # unmount in “reverse annoyances” order
+        for p in [
+            DST / "root" / "boot" / "efi",
+            DST / "efi",
+            DST / "boot",
+            DST / "root",
+            SRC / "root_top",
+            SRC / "root_sub_root",
+            SRC / "root_sub_home",
+            SRC / "root_sub_var",
+            SRC / "boot",
+            SRC / "efi",
+            SRC
+        ]:
+            umount(p)
+        # chroot bind mounts
+        for p in [
+            DST / "root" / "dev" / "pts",
+            DST / "root" / "dev",
+            DST / "root" / "proc",
+            DST / "root" / "sys",
+            DST / "root" / "run",
+            DST / "root" / "tmp"
+        ]:
+            umount(p)
+        if loopdev:
+            sh(["losetup", "-d", loopdev], check=False)
+            loopdev = None
+        udev_settle()
+
+    try:
+        banner("SAFETY CHECK: ABOUT TO ERASE TARGET DISK")
+        lsblk_tree(disk)
+        print(f"\nDisk: {disk} | Scheme: {args.scheme} | Data: {'yes' if args.make_data else 'no'} | Image: {image.name}")
+        print("Ctrl+C now if that's not MASH.\n")
+        time.sleep(5)
+
+        # ---- unmount anything on disk ----
+        banner("Unmounting anything using target disk")
+        mps = sh(["lsblk", "-lnpo", "MOUNTPOINT", disk], capture=True)
+        for mp in [x.strip() for x in mps.splitlines() if x.strip()]:
+            sh(["umount", "-R", mp], check=False)
+        cleanup()
+
+        # ---- wipe signatures ----
+        banner("Wiping signatures")
+        sh(["wipefs", "-a", disk], check=False)
+        udev_settle()
+
+        # ---- partition ----
+        banner(f"Partitioning ({args.scheme.upper()})")
+        efi_end_mib = parse_size_to_mib(args.efi_size)
+        boot_size_mib = parse_size_to_mib(args.boot_size)
+        boot_end_mib = efi_end_mib + boot_size_mib
+
+        efi_start = "4MiB"
+        efi_end = f"{efi_end_mib}MiB"
+        boot_start = efi_end
+        boot_end = f"{boot_end_mib}MiB"
+
+        if args.scheme == "gpt":
+            sh(["parted", "-s", disk, "mklabel", "gpt"])
+            sh(["parted", "-s", "-a", "optimal", disk, "mkpart", "primary", "fat32", efi_start, efi_end])
+            sh(["parted", "-s", disk, "set", "1", "esp", "on"])
+            sh(["parted", "-s", "-a", "optimal", disk, "mkpart", "primary", "ext4", boot_start, boot_end])
+            if args.make_data:
+                sh(["parted", "-s", "-a", "optimal", disk, "mkpart", "primary", "btrfs", boot_end, "70%"])
+                sh(["parted", "-s", "-a", "optimal", disk, "mkpart", "primary", "ext4", "70%", "100%"])
+            else:
+                sh(["parted", "-s", "-a", "optimal", disk, "mkpart", "primary", "btrfs", boot_end, "100%"])
+        else:
+            print("⚠️  MBR selected. On many 4TB USB drives this can fail due to msdos limits.")
+            sh(["parted", "-s", disk, "mklabel", "msdos"])
+            sh(["parted", "-s", "-a", "optimal", disk, "mkpart", "primary", "fat32", efi_start, efi_end])
+            sh(["parted", "-s", disk, "set", "1", "boot", "on"])
+            sh(["parted", "-s", disk, "set", "1", "lba", "on"])
+            sh(["parted", "-s", "-a", "optimal", disk, "mkpart", "primary", "ext4", boot_start, boot_end])
+            sh(["parted", "-s", "-a", "optimal", disk, "mkpart", "primary", "btrfs", boot_end, args.mbr_root_end])
+            if args.make_data:
+                sh(["parted", "-s", "-a", "optimal", disk, "mkpart", "primary", "ext4", args.mbr_root_end, "100%"])
+
+        sh(["parted", "-s", disk, "print"])
+        udev_settle()
+
+        efi_dev = f"{disk}1"
+        boot_dev = f"{disk}2"
+        root_dev = f"{disk}3"
+        data_dev = f"{disk}4" if args.make_data else None
+
+        # ---- format ----
... (diff truncated) ...
```

### holy-loop-fedora-ninja.py → holy-loop-fedora-ninja-final.py

```diff
--- holy-loop-fedora-ninja.py
+++ holy-loop-fedora-ninja-final.py
@@ -1,10 +1,34 @@
 #!/usr/bin/env python3
 """
-🎸 HOLY-LOOP-FEDORA-NINJA-V6: THE FINAL MASTERPIECE 🎷
-
-- Fixes the GRUB stub UUID mismatch (no more grub> prompt!).
-- Bridges Btrfs subvolumes for Dracut.
-- Perfectly merges PFTF UEFI and Fedora loaders.
+holy-loop-fedora-ninja-final.py
+
+Flash Fedora aarch64 RAW image onto a Pi4 USB disk so it boots via Pi4 UEFI (PFTF).
+
+What this script does (the “we learned this the hard way” version):
+- Partitions disk as:
+    p1 EFI  (FAT32)  1GiB  -> Pi firmware + PFTF UEFI + Fedora EFI loaders
+    p2 BOOT (ext4)   2GiB  -> Fedora /boot (kernel+initramfs+BLS)
+    p3 ROOT (btrfs)  rest  -> Fedora root filesystem (subvols root/home/var)
+    p4 DATA (ext4)   opt   -> optional data partition (GPT recommended)
+- Copies Fedora ROOT from image btrfs subvols: root + home + var (no “missing var/home” surprises)
+- Copies Fedora /boot partition from image -> real /boot (so dracut + BLS are sane)
+- Installs Pi4 UEFI firmware (PFTF) onto EFI (vfat-safe rsync: no chown/perms)
+- Merges Fedora EFI loaders (EFI/BOOT/BOOTAA64.EFI etc.) onto EFI
+- Writes a known-good config.txt for Pi4 UEFI (PFTF)
+- Creates /boot/efi mountpoint and writes UUID-based /etc/fstab
+- Patches BLS entries to point at the *new* root UUID and sets rootflags=subvol=root
+- Runs dracut in chroot with /boot + /boot/efi mounted, plus fixed /var/tmp and devpts
+
+Usage:
+  sudo ./holy-loop-fedora-ninja-final.py Fedora-KDE-Desktop-Disk-43-1.6.aarch64.raw \
+    --disk /dev/sda \
+    --uefi-dir ./rpi4uefi \
+    --scheme gpt \
+    --make-data
+
+Notes:
+- GPT is strongly recommended for 4TB disks. MBR can hit the 2TB/4TB geometry limits depending on the device.
+- Requires: parted, wipefs, mkfs.vfat, mkfs.ext4, mkfs.btrfs, losetup, rsync, blkid, btrfs, dracut, findmnt
 """

 import argparse
@@ -16,9 +40,12 @@
 import time
 from pathlib import Path

-# ---------- ⚡ NINJA HELPERS ⚡ ----------
-
+
+# ---------- helpers ----------
 def sh(cmd, check=True, capture=False):
+    """
+    Run command. cmd can be list or string.
+    """
     if isinstance(cmd, str):
         p = subprocess.run(cmd, shell=True, check=check,
                            stdout=subprocess.PIPE if capture else None,
@@ -33,132 +60,478 @@
         return (p.stdout or "").strip()
     return ""

-def is_actually_mounted(path):
-    return os.path.ismount(str(path))
-
-def banner(title, icon="🚀"):
-    width = 50
-    print(f"\n\033[95m╭{'─' * width}╮\033[0m")
-    print(f"\033[95m│\033[0m  {icon}  \033[1m{title.upper():<{width-8}}\033[0m \033[95m│\033[0m")
-    print(f"\033[95m╰{'─' * width}╯\033[0m")
-
-def rsync_progress(src, dst, desc):
-    banner(desc, icon="🚚")
-    cmd = ["rsync", "-aHAX", "--numeric-ids", "--info=progress2", f"{src}/", f"{dst}/"]
-    subprocess.run(cmd, check=True)
-
-def rsync_vfat_safe(src, dst, desc):
-    banner(desc, icon="💾")
-    cmd = ["rsync", "-rltD", "--no-owner", "--no-group", "--no-perms", f"{src}/", f"{dst}/"]
+
+def need(binname: str):
+    if shutil.which(binname) is None:
+        die(f"Missing required command: {binname}")
+
+
+def die(msg: str, code: int = 1):
+    print(f"\n[FATAL] {msg}\n")
+    sys.exit(code)
+
+
+def banner(title: str):
+    line = "─" * (len(title) + 4)
+    print(f"\n╭{line}╮")
+    print(f"│  {title}  │")
+    print(f"╰{line}╯")
+
+
+def mkdirp(p: Path):
+    p.mkdir(parents=True, exist_ok=True)
+
+
+def umount(path: Path):
+    sh(["umount", "-R", str(path)], check=False)
+
+
+def udev_settle():
+    sh(["udevadm", "settle"], check=False)
+
+
+def lsblk_tree(disk: str):
+    sh(["lsblk", "-o", "NAME,SIZE,TYPE,FSTYPE,MOUNTPOINTS,MODEL", disk], check=False)
+
+
+def blkid_uuid(dev: str) -> str:
+    return sh(["blkid", "-s", "UUID", "-o", "value", dev], capture=True)
+
+
+def parse_size_to_mib(s: str) -> int:
+    s = s.strip()
+    m = re.match(r"^(\d+)\s*(MiB|GiB)$", s, re.I)
+    if not m:
+        die(f"Size must be like 1024MiB or 2GiB, got: {s}")
+    v = int(m.group(1))
+    unit = m.group(2).lower()
+    return v if unit == "mib" else v * 1024
+
+
+def rsync_progress(src: Path, dst: Path, desc: str, extra_args=None):
+    """
+    rsync with a simple progress bar using --info=progress2
+    """
+    extra_args = extra_args or []
+    banner(desc)
+    cmd = ["rsync", "-aHAX", "--numeric-ids", "--info=progress2"] + extra_args + [f"{src}/", f"{dst}/"]
+    proc = subprocess.Popen(cmd, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True)
+    barw = 28
+    last = -1
+    try:
+        for line in proc.stdout:
+            m = re.search(r"\s(\d{1,3})%\s", line)
+            if m:
+                pct = int(m.group(1))
+                if pct != last:
+                    last = pct
+                    filled = int((pct / 100) * barw)
+                    bar = "█" * filled + " " * (barw - filled)
+                    sys.stdout.write(f"\r[{bar}] {pct:3d}%")
+                    sys.stdout.flush()
+        rc = proc.wait()
+        if last >= 0:
+            sys.stdout.write("\r" + " " * (barw + 10) + "\r")
+        if rc != 0:
+            die(f"{desc} failed (exit {rc})")
+        print("✅ Done.")
+    finally:
+        try:
+            proc.kill()
+        except Exception:
+            pass
+
+
+def rsync_vfat_safe(src: Path, dst: Path, desc: str):
+    """
+    VFAT cannot chown; do a safe copy with no owners/groups/perms.
+    """
+    banner(desc)
+    cmd = ["rsync", "-rltD", "--delete", "--no-owner", "--no-group", "--no-perms", f"{src}/", f"{dst}/"]
     sh(cmd, check=True)

-# ---------- 🏗️ MAIN LOGIC 🏗️ ----------
+
+def write_file(path: Path, content: str):
+    path.write_text(content, encoding="utf-8")
+
+
+def patch_bls_entries(boot_entries_dir: Path, root_uuid: str):
+    """
+    Patch BLS entries under /boot/loader/entries/*.conf:
+      - set root=UUID=<new>
+      - ensure rootflags=subvol=root
+    """
+    if not boot_entries_dir.exists():
+        print(f"⚠️  No BLS entries dir found: {boot_entries_dir} (skipping BLS patch)")
+        return
+
+    files = sorted(boot_entries_dir.glob("*.conf"))
+    if not files:
+        print(f"⚠️  No BLS entry files in {boot_entries_dir} (skipping BLS patch)")
+        return
+
+    print(f"🩹 Patching BLS entries in {boot_entries_dir} ...")
+    for f in files:
+        txt = f.read_text(encoding="utf-8", errors="ignore").splitlines(True)
+
+        out = []
+        for line in txt:
+            if line.startswith("options "):
+                opts = line[len("options "):].strip()
+
+                # replace root=...
+                if re.search(r"\broot=UUID=[0-9a-fA-F-]+\b", opts):
+                    opts = re.sub(r"\broot=UUID=[0-9a-fA-F-]+\b", f"root=UUID={root_uuid}", opts)
+                elif re.search(r"\broot=[^\s]+\b", opts):
+                    opts = re.sub(r"\broot=[^\s]+\b", f"root=UUID={root_uuid}", opts)
+                else:
+                    opts = f"root=UUID={root_uuid} " + opts
+
+                # ensure rootflags=subvol=root
+                if re.search(r"\brootflags=", opts):
+                    # overwrite whatever is there
+                    opts = re.sub(r"\brootflags=[^\s]+\b", "rootflags=subvol=root", opts)
+                else:
+                    opts = opts + " rootflags=subvol=root"
+
+                out.append("options " + opts.strip() + "\n")
+            else:
+                out.append(line)
+
+        f.write_text("".join(out), encoding="utf-8")
+    print("✅ BLS patched.")
+

 def main():
+    if os.geteuid() != 0:
+        die("Run as root: sudo ./holy-loop-fedora-ninja-final.py ...")
+
     ap = argparse.ArgumentParser()
     ap.add_argument("image", help="Fedora *.raw image")
-    ap.add_argument("--disk", default="/dev/sda", help="Target disk")
-    ap.add_argument("--uefi-dir", default="./rpi4uefi", help="PFTF UEFI dir")
+    ap.add_argument("--disk", default="/dev/sda", help="Target disk (MASH)")
+    ap.add_argument("--uefi-dir", default="./rpi4uefi", help="PFTF UEFI dir (must contain RPI_EFI.fd)")
+    ap.add_argument("--scheme", choices=["gpt", "mbr"], default="gpt", help="Partition scheme (gpt recommended)")
+    ap.add_argument("--make-data", action="store_true", help="Create /data partition (GPT recommended)")
+    ap.add_argument("--efi-size", default="1024MiB", help="EFI size (default 1024MiB)")
+    ap.add_argument("--boot-size", default="2048MiB", help="/boot size (default 2048MiB)")
+    ap.add_argument("--mbr-root-end", default="1800GiB", help="MBR-only: end of ROOT partition (default 1800GiB)")
+    ap.add_argument("--no-dracut", action="store_true", help="Skip dracut (not recommended)")
     args = ap.parse_args()

-    image, disk = Path(args.image).resolve(), args.disk
+    image = Path(args.image).resolve()
+    disk = args.disk
     uefi_dir = Path(args.uefi_dir).resolve()
-    SRC, DST = Path("/mnt/ninja_src"), Path("/mnt/ninja_dst")
+
+    for c in ["parted", "wipefs", "mkfs.vfat", "mkfs.ext4", "mkfs.btrfs", "losetup", "rsync", "blkid", "btrfs", "dracut", "findmnt"]:
+        need(c)
+
+    if not image.exists():
+        die(f"Image not found: {image}")
+    if not Path(disk).exists():
+        die(f"Disk not found: {disk}")
+    if not (uefi_dir / "RPI_EFI.fd").exists():
+        die(f"Missing {uefi_dir}/RPI_EFI.fd")
+
+    # mount roots
+    SRC = Path("/mnt/ninja_src")
+    DST = Path("/mnt/ninja_dst")
+
     loopdev = None

     def cleanup():
         nonlocal loopdev
-        print("\n\033[93m🧹 Sweeping up the dojo...\033[0m")
-        mounts = [
-            DST/"root_sub_root/boot/efi", DST/"root_sub_root/boot",
-            DST/"root_sub_root/var", DST/"root_sub_root/dev/pts",
-            DST/"root_sub_root/dev", DST/"root_sub_root/proc",
-            DST/"root_sub_root/sys", DST/"root_sub_root/run",
-            DST/"root_sub_root", DST/"root_top", DST/"efi", DST/"boot",
-            SRC/"root_sub_root", SRC/"root_sub_home", SRC/"root_sub_var",
-            SRC/"root_top", SRC/"boot", SRC/"efi"
+        # unmount in “reverse annoyances” order
+        for p in [
+            DST / "root" / "boot" / "efi",
+            DST / "efi",
+            DST / "boot",
+            DST / "root",
+            SRC / "root_top",
+            SRC / "root_sub_root",
+            SRC / "root_sub_home",
+            SRC / "root_sub_var",
+            SRC / "boot",
+            SRC / "efi",
+            SRC
+        ]:
+            umount(p)
+        # chroot bind mounts
+        for p in [
+            DST / "root" / "dev" / "pts",
+            DST / "root" / "dev",
+            DST / "root" / "proc",
+            DST / "root" / "sys",
+            DST / "root" / "run",
+            DST / "root" / "tmp"
+        ]:
+            umount(p)
+        if loopdev:
+            sh(["losetup", "-d", loopdev], check=False)
+            loopdev = None
+        udev_settle()
+
+    try:
+        banner("SAFETY CHECK: ABOUT TO ERASE TARGET DISK")
+        lsblk_tree(disk)
+        print(f"\nDisk: {disk} | Scheme: {args.scheme} | Data: {'yes' if args.make_data else 'no'} | Image: {image.name}")
+        print("Ctrl+C now if that's not MASH.\n")
+        time.sleep(5)
+
+        # ---- unmount anything on disk ----
+        banner("Unmounting anything using target disk")
+        mps = sh(["lsblk", "-lnpo", "MOUNTPOINT", disk], capture=True)
+        for mp in [x.strip() for x in mps.splitlines() if x.strip()]:
+            sh(["umount", "-R", mp], check=False)
+        cleanup()
+
+        # ---- wipe signatures ----
+        banner("Wiping signatures")
+        sh(["wipefs", "-a", disk], check=False)
+        udev_settle()
+
+        # ---- partition ----
+        banner(f"Partitioning ({args.scheme.upper()})")
+        efi_end_mib = parse_size_to_mib(args.efi_size)
+        boot_size_mib = parse_size_to_mib(args.boot_size)
+        boot_end_mib = efi_end_mib + boot_size_mib
+
+        efi_start = "4MiB"
+        efi_end = f"{efi_end_mib}MiB"
+        boot_start = efi_end
+        boot_end = f"{boot_end_mib}MiB"
+
+        if args.scheme == "gpt":
+            sh(["parted", "-s", disk, "mklabel", "gpt"])
+            sh(["parted", "-s", "-a", "optimal", disk, "mkpart", "primary", "fat32", efi_start, efi_end])
+            sh(["parted", "-s", disk, "set", "1", "esp", "on"])
+            sh(["parted", "-s", "-a", "optimal", disk, "mkpart", "primary", "ext4", boot_start, boot_end])
+            if args.make_data:
+                sh(["parted", "-s", "-a", "optimal", disk, "mkpart", "primary", "btrfs", boot_end, "70%"])
+                sh(["parted", "-s", "-a", "optimal", disk, "mkpart", "primary", "ext4", "70%", "100%"])
+            else:
+                sh(["parted", "-s", "-a", "optimal", disk, "mkpart", "primary", "btrfs", boot_end, "100%"])
+        else:
+            print("⚠️  MBR selected. On many 4TB USB drives this can fail due to msdos limits.")
+            sh(["parted", "-s", disk, "mklabel", "msdos"])
+            sh(["parted", "-s", "-a", "optimal", disk, "mkpart", "primary", "fat32", efi_start, efi_end])
+            sh(["parted", "-s", disk, "set", "1", "boot", "on"])
+            sh(["parted", "-s", disk, "set", "1", "lba", "on"])
+            sh(["parted", "-s", "-a", "optimal", disk, "mkpart", "primary", "ext4", boot_start, boot_end])
+            sh(["parted", "-s", "-a", "optimal", disk, "mkpart", "primary", "btrfs", boot_end, args.mbr_root_end])
+            if args.make_data:
+                sh(["parted", "-s", "-a", "optimal", disk, "mkpart", "primary", "ext4", args.mbr_root_end, "100%"])
+
+        sh(["parted", "-s", disk, "print"])
+        udev_settle()
+
+        efi_dev = f"{disk}1"
+        boot_dev = f"{disk}2"
+        root_dev = f"{disk}3"
+        data_dev = f"{disk}4" if args.make_data else None
+
+        # ---- format ----
+        banner("Formatting filesystems")
+        sh(["mkfs.vfat", "-F", "32", "-n", "EFI", efi_dev])
+        sh(["mkfs.ext4", "-F", "-L", "BOOT", boot_dev])
+        sh(["mkfs.btrfs", "-f", "-L", "FEDORA", root_dev])
+        if data_dev:
+            sh(["mkfs.ext4", "-F", "-L", "DATA", data_dev])
+
+        udev_settle()
+
+        # ---- loop mount image ----
+        banner("Loop-mounting Fedora image")
+        loopdev = sh(["losetup", "--show", "-Pf", str(image)], capture=True)
+        sh(["lsblk", loopdev])
+
+        img_efi = f"{loopdev}p1"
+        img_boot = f"{loopdev}p2"
+        img_root = f"{loopdev}p3"
+
+        # ---- mount sources ----
+        banner("Mounting image partitions")
+        mkdirp(SRC / "efi")
+        mkdirp(SRC / "boot")
+        mkdirp(SRC / "root_top")
+        mkdirp(SRC / "root_sub_root")
+        mkdirp(SRC / "root_sub_home")
+        mkdirp(SRC / "root_sub_var")
+
+        sh(["mount", img_efi, str(SRC / "efi")])
+        sh(["mount", img_boot, str(SRC / "boot")])
+        sh(["mount", "-t", "btrfs", img_root, str(SRC / "root_top")])
+
+        subvols = sh(["btrfs", "subvolume", "list", str(SRC / "root_top")], capture=True)
+        has_root = re.search(r"\bpath\s+root$", subvols, re.M) is not None
+        has_home = re.search(r"\bpath\s+home$", subvols, re.M) is not None
+        has_var = re.search(r"\bpath\s+var$", subvols, re.M) is not None
+        if not has_root:
+            die("Image does not contain btrfs subvol 'root' (unexpected for Fedora RAW)")
+
... (diff truncated) ...
```

### holy-loop-fedora-ninja.py → holy-loop-fedora-ninja-final.py

```diff
--- holy-loop-fedora-ninja.py
+++ holy-loop-fedora-ninja-final.py
@@ -1,120 +1,537 @@
 #!/usr/bin/env python3
 """
-🎸 HOLY-LOOP-FEDORA-NINJA (ULTIMATE MBR EDITION) 🎷
-- Combines 4TB MBR logic with BLS patching and Nuclear Unmounts.
-- Fixes the 'stuck at GRUB menu' issue by updating kernel arguments.
+holy-loop-fedora-ninja-final.py
+
+Flash Fedora aarch64 RAW image onto a Pi4 USB disk so it boots via Pi4 UEFI (PFTF).
+
+What this script does (the “we learned this the hard way” version):
+- Partitions disk as:
+    p1 EFI  (FAT32)  1GiB  -> Pi firmware + PFTF UEFI + Fedora EFI loaders
+    p2 BOOT (ext4)   2GiB  -> Fedora /boot (kernel+initramfs+BLS)
+    p3 ROOT (btrfs)  rest  -> Fedora root filesystem (subvols root/home/var)
+    p4 DATA (ext4)   opt   -> optional data partition (GPT recommended)
+- Copies Fedora ROOT from image btrfs subvols: root + home + var (no “missing var/home” surprises)
+- Copies Fedora /boot partition from image -> real /boot (so dracut + BLS are sane)
+- Installs Pi4 UEFI firmware (PFTF) onto EFI (vfat-safe rsync: no chown/perms)
+- Merges Fedora EFI loaders (EFI/BOOT/BOOTAA64.EFI etc.) onto EFI
+- Writes a known-good config.txt for Pi4 UEFI (PFTF)
+- Creates /boot/efi mountpoint and writes UUID-based /etc/fstab
+- Patches BLS entries to point at the *new* root UUID and sets rootflags=subvol=root
+- Runs dracut in chroot with /boot + /boot/efi mounted, plus fixed /var/tmp and devpts
+
+Usage:
+  sudo ./holy-loop-fedora-ninja-final.py Fedora-KDE-Desktop-Disk-43-1.6.aarch64.raw \
+    --disk /dev/sda \
+    --uefi-dir ./rpi4uefi \
+    --scheme gpt \
+    --make-data
+
+Notes:
+- GPT is strongly recommended for 4TB disks. MBR can hit the 2TB/4TB geometry limits depending on the device.
+- Requires: parted, wipefs, mkfs.vfat, mkfs.ext4, mkfs.btrfs, losetup, rsync, blkid, btrfs, dracut, findmnt
 """

+import argparse
 import os
 import re
+import shutil
 import subprocess
+import sys
 import time
 from pathlib import Path

-# ---------- 🔧 EDIT THESE ----------
-IMAGE    = "Fedora-KDE-Desktop-Disk-43-1.6.aarch64.raw"
-DISK     = "/dev/sda"
-UEFI_DIR = "./rpi4uefi"
-
-# ---------- ⚡ NINJA HELPERS ⚡ ----------
+
+# ---------- helpers ----------
 def sh(cmd, check=True, capture=False):
-    shell = isinstance(cmd, str)
-    p = subprocess.run(cmd, shell=shell, check=check,
-                       stdout=subprocess.PIPE if capture else None,
-                       stderr=subprocess.PIPE if capture else None, text=True)
-    return p.stdout.strip() if capture else ""
-
-def banner(title, icon="🚀"):
-    print(f"\n\033[95m╭{'─' * 50}╮\n│  {icon}  {title.upper():<42} │\n╰{'─' * 50}╯\033[0m")
-
-def rsync_progress(src, dst, desc):
-    banner(desc, icon="🚚")
-    subprocess.run(["rsync", "-aHAX", "--numeric-ids", "--info=progress2", f"{src}/", f"{dst}/"], check=True)
-
-def patch_bls_entries(boot_entries_dir, root_uuid):
-    """Rewrites kernel boot files to use the new drive's UUID."""
-    if not boot_entries_dir.exists(): return
-    print(f"🩹 Patching BLS entries in {boot_entries_dir}...")
-    for f in boot_entries_dir.glob("*.conf"):
-        content = f.read_text()
-        # Update root UUID and ensure Btrfs flags are present
-        content = re.sub(r"root=UUID=[0-9a-fA-F-]+", f"root=UUID={root_uuid}", content)
-        if "rootflags=subvol=root" not in content:
-            content = content.replace("options ", "options rootflags=subvol=root ")
-        f.write_text(content)
+    """
+    Run command. cmd can be list or string.
+    """
+    if isinstance(cmd, str):
+        p = subprocess.run(cmd, shell=True, check=check,
+                           stdout=subprocess.PIPE if capture else None,
+                           stderr=subprocess.PIPE if capture else None,
+                           text=True)
+    else:
+        p = subprocess.run(cmd, shell=False, check=check,
+                           stdout=subprocess.PIPE if capture else None,
+                           stderr=subprocess.PIPE if capture else None,
+                           text=True)
+    if capture:
+        return (p.stdout or "").strip()
+    return ""
+
+
+def need(binname: str):
+    if shutil.which(binname) is None:
+        die(f"Missing required command: {binname}")
+
+
+def die(msg: str, code: int = 1):
+    print(f"\n[FATAL] {msg}\n")
+    sys.exit(code)
+
+
+def banner(title: str):
+    line = "─" * (len(title) + 4)
+    print(f"\n╭{line}╮")
+    print(f"│  {title}  │")
+    print(f"╰{line}╯")
+
+
+def mkdirp(p: Path):
+    p.mkdir(parents=True, exist_ok=True)
+
+
+def umount(path: Path):
+    sh(["umount", "-R", str(path)], check=False)
+
+
+def udev_settle():
+    sh(["udevadm", "settle"], check=False)
+
+
+def lsblk_tree(disk: str):
+    sh(["lsblk", "-o", "NAME,SIZE,TYPE,FSTYPE,MOUNTPOINTS,MODEL", disk], check=False)
+
+
+def blkid_uuid(dev: str) -> str:
+    return sh(["blkid", "-s", "UUID", "-o", "value", dev], capture=True)
+
+
+def parse_size_to_mib(s: str) -> int:
+    s = s.strip()
+    m = re.match(r"^(\d+)\s*(MiB|GiB)$", s, re.I)
+    if not m:
+        die(f"Size must be like 1024MiB or 2GiB, got: {s}")
+    v = int(m.group(1))
+    unit = m.group(2).lower()
+    return v if unit == "mib" else v * 1024
+
+
+def rsync_progress(src: Path, dst: Path, desc: str, extra_args=None):
+    """
+    rsync with a simple progress bar using --info=progress2
+    """
+    extra_args = extra_args or []
+    banner(desc)
+    cmd = ["rsync", "-aHAX", "--numeric-ids", "--info=progress2"] + extra_args + [f"{src}/", f"{dst}/"]
+    proc = subprocess.Popen(cmd, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True)
+    barw = 28
+    last = -1
+    try:
+        for line in proc.stdout:
+            m = re.search(r"\s(\d{1,3})%\s", line)
+            if m:
+                pct = int(m.group(1))
+                if pct != last:
+                    last = pct
+                    filled = int((pct / 100) * barw)
+                    bar = "█" * filled + " " * (barw - filled)
+                    sys.stdout.write(f"\r[{bar}] {pct:3d}%")
+                    sys.stdout.flush()
+        rc = proc.wait()
+        if last >= 0:
+            sys.stdout.write("\r" + " " * (barw + 10) + "\r")
+        if rc != 0:
+            die(f"{desc} failed (exit {rc})")
+        print("✅ Done.")
+    finally:
+        try:
+            proc.kill()
+        except Exception:
+            pass
+
+
+def rsync_vfat_safe(src: Path, dst: Path, desc: str):
+    """
+    VFAT cannot chown; do a safe copy with no owners/groups/perms.
+    """
+    banner(desc)
+    cmd = ["rsync", "-rltD", "--delete", "--no-owner", "--no-group", "--no-perms", f"{src}/", f"{dst}/"]
+    sh(cmd, check=True)
+
+
+def write_file(path: Path, content: str):
+    path.write_text(content, encoding="utf-8")
+
+
+def patch_bls_entries(boot_entries_dir: Path, root_uuid: str):
+    """
+    Patch BLS entries under /boot/loader/entries/*.conf:
+      - set root=UUID=<new>
+      - ensure rootflags=subvol=root
+    """
+    if not boot_entries_dir.exists():
+        print(f"⚠️  No BLS entries dir found: {boot_entries_dir} (skipping BLS patch)")
+        return
+
+    files = sorted(boot_entries_dir.glob("*.conf"))
+    if not files:
+        print(f"⚠️  No BLS entry files in {boot_entries_dir} (skipping BLS patch)")
+        return
+
+    print(f"🩹 Patching BLS entries in {boot_entries_dir} ...")
+    for f in files:
+        txt = f.read_text(encoding="utf-8", errors="ignore").splitlines(True)
+
+        out = []
+        for line in txt:
+            if line.startswith("options "):
+                opts = line[len("options "):].strip()
+
+                # replace root=...
+                if re.search(r"\broot=UUID=[0-9a-fA-F-]+\b", opts):
+                    opts = re.sub(r"\broot=UUID=[0-9a-fA-F-]+\b", f"root=UUID={root_uuid}", opts)
+                elif re.search(r"\broot=[^\s]+\b", opts):
+                    opts = re.sub(r"\broot=[^\s]+\b", f"root=UUID={root_uuid}", opts)
+                else:
+                    opts = f"root=UUID={root_uuid} " + opts
+
+                # ensure rootflags=subvol=root
+                if re.search(r"\brootflags=", opts):
+                    # overwrite whatever is there
+                    opts = re.sub(r"\brootflags=[^\s]+\b", "rootflags=subvol=root", opts)
+                else:
+                    opts = opts + " rootflags=subvol=root"
+
+                out.append("options " + opts.strip() + "\n")
+            else:
+                out.append(line)
+
+        f.write_text("".join(out), encoding="utf-8")
+    print("✅ BLS patched.")
+

 def main():
-    SRC, DST = Path("/mnt/ninja_src"), Path("/mnt/ninja_dst")
-    img_path = Path(IMAGE).resolve()
+    if os.geteuid() != 0:
+        die("Run as root: sudo ./holy-loop-fedora-ninja-final.py ...")
+
+    ap = argparse.ArgumentParser()
+    ap.add_argument("image", help="Fedora *.raw image")
+    ap.add_argument("--disk", default="/dev/sda", help="Target disk (MASH)")
+    ap.add_argument("--uefi-dir", default="./rpi4uefi", help="PFTF UEFI dir (must contain RPI_EFI.fd)")
+    ap.add_argument("--scheme", choices=["gpt", "mbr"], default="gpt", help="Partition scheme (gpt recommended)")
+    ap.add_argument("--make-data", action="store_true", help="Create /data partition (GPT recommended)")
+    ap.add_argument("--efi-size", default="1024MiB", help="EFI size (default 1024MiB)")
+    ap.add_argument("--boot-size", default="2048MiB", help="/boot size (default 2048MiB)")
+    ap.add_argument("--mbr-root-end", default="1800GiB", help="MBR-only: end of ROOT partition (default 1800GiB)")
+    ap.add_argument("--no-dracut", action="store_true", help="Skip dracut (not recommended)")
+    args = ap.parse_args()
+
+    image = Path(args.image).resolve()
+    disk = args.disk
+    uefi_dir = Path(args.uefi_dir).resolve()
+
+    for c in ["parted", "wipefs", "mkfs.vfat", "mkfs.ext4", "mkfs.btrfs", "losetup", "rsync", "blkid", "btrfs", "dracut", "findmnt"]:
+        need(c)
+
+    if not image.exists():
+        die(f"Image not found: {image}")
+    if not Path(disk).exists():
+        die(f"Disk not found: {disk}")
+    if not (uefi_dir / "RPI_EFI.fd").exists():
+        die(f"Missing {uefi_dir}/RPI_EFI.fd")
+
+    # mount roots
+    SRC = Path("/mnt/ninja_src")
+    DST = Path("/mnt/ninja_dst")
+
     loopdev = None

+    def cleanup():
+        nonlocal loopdev
+        # unmount in “reverse annoyances” order
+        for p in [
+            DST / "root" / "boot" / "efi",
+            DST / "efi",
+            DST / "boot",
+            DST / "root",
+            SRC / "root_top",
+            SRC / "root_sub_root",
+            SRC / "root_sub_home",
+            SRC / "root_sub_var",
+            SRC / "boot",
+            SRC / "efi",
+            SRC
+        ]:
+            umount(p)
+        # chroot bind mounts
+        for p in [
+            DST / "root" / "dev" / "pts",
+            DST / "root" / "dev",
+            DST / "root" / "proc",
+            DST / "root" / "sys",
+            DST / "root" / "run",
+            DST / "root" / "tmp"
+        ]:
+            umount(p)
+        if loopdev:
+            sh(["losetup", "-d", loopdev], check=False)
+            loopdev = None
+        udev_settle()
+
     try:
-        banner("Nuclear Unmount", icon="☢️")
-        for _ in range(2):
-            mps = sh(f"lsblk -lnpo MOUNTPOINT {DISK}", capture=True)
-            for mp in sorted([x.strip() for x in mps.splitlines() if x.strip()], reverse=True):
-                sh(f"umount -f -l {mp}", check=False)
-
-        banner("Wiping & Partitioning", icon="🏗️")
-        sh(f"wipefs -a {DISK}")
-        sh(f"parted -s {DISK} mklabel msdos")
-        sh(f"parted -s {DISK} unit MiB mkpart primary fat32 4 512")
-        sh(f"parted -s {DISK} set 1 boot on")
-        sh(f"parted -s {DISK} unit MiB mkpart primary ext4 512 2560")
-        sh(f"parted -s {DISK} unit MiB mkpart primary btrfs 2560 1900000")
-        sh(f"parted -s {DISK} unit MiB mkpart primary ext4 1900000 100%")
-
-        banner("Formatting", icon="🛠️")
-        sh(f"mkfs.vfat -F 32 -n EFI {DISK}1")
-        sh(f"mkfs.ext4 -F -L BOOT {DISK}2")
-        sh(f"mkfs.btrfs -f -L ROOT {DISK}3")
-        sh(f"mkfs.ext4 -F -L DATA {DISK}4")
-
-        loopdev = sh(f"losetup --show -Pf {img_path}", capture=True)
-        for p in [SRC/"efi", SRC/"boot", SRC/"root_top", DST/"efi", DST/"boot", DST/"root_top"]: p.mkdir(parents=True, exist_ok=True)
-
-        # Mounting
-        sh(f"mount {DISK}1 {DST}/efi"); sh(f"mount {DISK}2 {DST}/boot"); sh(f"mount -t btrfs {DISK}3 {DST}/root_top")
-        sh(f"mount {loopdev}p1 {SRC}/efi"); sh(f"mount {loopdev}p2 {SRC}/boot"); sh(f"mount -t btrfs {loopdev}p3 {SRC}/root_top")
-
-        # Cloning Subvolumes
-        for sub in ["root", "home", "var"]:
-            sh(f"btrfs subvolume create {DST}/root_top/{sub}")
-            target = DST/f"sub_{sub}"
-            target.mkdir(exist_ok=True)
-            sh(f"mount -t btrfs -o subvol={sub} {DISK}3 {target}")
-            source = SRC/f"sub_{sub}"
-            source.mkdir(exist_ok=True)
-            sh(f"mount -t btrfs -o subvol={sub} {loopdev}p3 {source}")
-            rsync_progress(source, target, f"Cloning {sub}")
-
-        rsync_progress(SRC/"boot", DST/"boot", "Cloning /boot")
-
-        banner("Merging EFI & PFTF", icon="🥧")
-        sh(f"rsync -rltD --no-owner --no-group --no-perms {SRC}/efi/EFI/ {DST}/efi/EFI/")
-        sh(f"rsync -rltD --no-owner --no-group --no-perms {UEFI_DIR}/ {DST}/efi/")
-
-        banner("Fixing Boot Identities", icon="🩹")
-        boot_uuid = sh(f"blkid -s UUID -o value {DISK}2", capture=True)
-        root_uuid = sh(f"blkid -s UUID -o value {DISK}3", capture=True)
-
-        # Patch GRUB Stub (the "redirector" file)
-        stub = f"search --no-floppy --fs-uuid --set=dev {boot_uuid}\nset prefix=($dev)/grub2\nconfigfile $prefix/grub.cfg\n"
-        (DST/"efi/EFI/fedora/grub.cfg").write_text(stub)
-
-        # Patch BLS Entries (the kernel arguments)
-        patch_bls_entries(DST/"boot/loader/entries", root_uuid)
-
-        banner("Finalizing", icon="🏁")
-        # Write FSTAB with new UUIDs
-        fstab = f"UUID={root_uuid} / btrfs subvol=root,compress=zstd:1 0 0\nUUID={boot_uuid} /boot ext4 defaults 0 2\n"
-        (DST/"sub_root/etc/fstab").write_text(fstab)
-
-        print("✅ Mission Accomplished. Your 4TB drive is now a self-aware Fedora Ninja.")
+        banner("SAFETY CHECK: ABOUT TO ERASE TARGET DISK")
+        lsblk_tree(disk)
+        print(f"\nDisk: {disk} | Scheme: {args.scheme} | Data: {'yes' if args.make_data else 'no'} | Image: {image.name}")
+        print("Ctrl+C now if that's not MASH.\n")
+        time.sleep(5)
+
+        # ---- unmount anything on disk ----
+        banner("Unmounting anything using target disk")
+        mps = sh(["lsblk", "-lnpo", "MOUNTPOINT", disk], capture=True)
+        for mp in [x.strip() for x in mps.splitlines() if x.strip()]:
+            sh(["umount", "-R", mp], check=False)
+        cleanup()
+
+        # ---- wipe signatures ----
+        banner("Wiping signatures")
+        sh(["wipefs", "-a", disk], check=False)
+        udev_settle()
+
+        # ---- partition ----
+        banner(f"Partitioning ({args.scheme.upper()})")
+        efi_end_mib = parse_size_to_mib(args.efi_size)
+        boot_size_mib = parse_size_to_mib(args.boot_size)
+        boot_end_mib = efi_end_mib + boot_size_mib
+
+        efi_start = "4MiB"
+        efi_end = f"{efi_end_mib}MiB"
+        boot_start = efi_end
... (diff truncated) ...
```
