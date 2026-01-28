#!/usr/bin/env python3
"""
mash_full_loop.py — Full-loop Fedora install orchestrator for Pi4 + MASH 4TB.

- Uses ninja-mbr4-v2.py (MBR-first, 4 partitions) to flash an image.
- Stages bootstrap to the new DATA partition: /data/mash-staging (LABEL=DATA).
- Applies offline locale defaults (en_GB, gb) into the target root.
- Installs a *one-shot first-boot* systemd unit in the target so the bootstrap runs automatically.

No user/password creation. First-boot wizard stays in charge.
"""
from __future__ import annotations
import argparse, os, shutil, subprocess, sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
NINJA = HERE / "ninja-mbr4-v2.py"
BOOTSTRAP_SRC = HERE  # contains mash_forge.py + helpers/ + wheel/

LOCALE_LANG = "en_GB.UTF-8"
VCONSOLE_KEYMAP = "gb"

DEFAULT_FIRSTBOOT_USER = "DrTweak"
DEFAULT_FIRSTBOOT_ARGS = ["--argon-one", "--zsh-starship", "--screensaver-nuke", "--with-starship-fallback", "--brave"]

def banner(msg: str):
    print("\n" + "="*88)
    print(msg)
    print("="*88)

def die(msg: str, code: int = 1):
    print(f"\n[ERROR] {msg}")
    raise SystemExit(code)

def preflight(args):
    banner("🧪 Preflight checks")
    # Must be root (we do destructive disk ops)
    if os.geteuid() != 0:
        die("Run with sudo/root.")

    img = os.path.expanduser(args.image)
    if not os.path.exists(img):
        die(f"Image not found: {img}")

    if not os.path.exists(args.disk) or not os.path.exists(f"/sys/class/block/{os.path.basename(args.disk)}"):
        die(f"Disk not found or not a block device: {args.disk}")

    # Very basic self-protection
    if os.path.realpath(args.disk) in ["/dev/mmcblk0", "/dev/nvme0n1"]:
        print("⚠️  WARNING: target disk looks like an internal/system disk. Double-check!")

    uefi_dir = os.path.expanduser(args.uefi_dir) if args.uefi_dir else ""
    if not uefi_dir or not os.path.isdir(uefi_dir):
        die(f"UEFI dir not found: {uefi_dir}")
    required = ["RPI_EFI.fd", "start4.elf", "fixup4.dat"]
    missing=[f for f in required if not os.path.exists(os.path.join(uefi_dir, f))]
    if missing:
        die(f"UEFI dir missing files: {', '.join(missing)}")

    # Required host tools (best effort)
    need_bins=["lsblk","wipefs","parted","mkfs.vfat","mkfs.ext4","mkfs.btrfs","rsync","losetup","mount","umount"]
    missing_bins=[b for b in need_bins if shutil.which(b) is None]
    if missing_bins:
        print(f"⚠️  Missing tools on host (install these): {' '.join(missing_bins)}")

    print("✅ Preflight OK (carry on, but you still own the safety check).")

def sh(cmd, check=True, **kwargs):
    if isinstance(cmd, str):
        return subprocess.run(cmd, shell=True, check=check, **kwargs)
    return subprocess.run(cmd, check=check, **kwargs)

def rsync_tree(src: Path, dst: Path):
    dst.mkdir(parents=True, exist_ok=True)
    sh(["rsync", "-a", f"{src}/", f"{dst}/"], check=True)

def mount_by_label(label: str, mountpoint: Path, dry_run: bool = False, opts: str = "defaults") -> bool:
    """Mount a block device by filesystem LABEL to mountpoint. Returns True on success."""
    mountpoint.mkdir(parents=True, exist_ok=True)
    # Resolve device by label (works on Fedora + Debian)
    dev = None
    try:
        p = subprocess.run(["blkid", "-L", label], capture_output=True, text=True)
        if p.returncode == 0:
            dev = p.stdout.strip()
    except Exception:
        dev = None
    if not dev:
        bylabel = Path("/dev/disk/by-label") / label
        if bylabel.exists():
            dev = str(bylabel.resolve())
    if not dev:
        print(f"[WARN] Could not resolve LABEL={label}")
        return False
    if dry_run:
        print(f"DRY-RUN: mount -o {opts} {dev} {mountpoint}")
        return True
    # Best-effort unmount if already mounted elsewhere
    subprocess.run(["umount", "-R", str(mountpoint)], check=False)
    r = subprocess.run(["mount", "-o", opts, dev, str(mountpoint)], check=False)
    if r.returncode != 0:
        print(f"[WARN] mount failed for {dev} -> {mountpoint}")
        return False
    return True


def mount_root_subvol(mnt_root: Path, disk: str = "/dev/sda", dry_run: bool = False) -> bool:
    """Mount the installed Fedora root (btrfs subvol=root) plus /boot and /boot/efi for offline edits.
    Returns True if mounted (or would mount in dry-run).
    """
    # We expect labels from the partitioner: sda1=EFI(vfat), sda2=BOOT(ext4), sda3=FEDORA(btrfs)
    banner("Mounting ROOT subvol for offline edits (LABEL=FEDORA, subvol=root)")
    if dry_run:
        print(f"DRY-RUN: mkdir -p {mnt_root}")
        print(f"DRY-RUN: umount -R {mnt_root}  (best-effort)")
        print(f"DRY-RUN: mount -o subvol=root LABEL=FEDORA {mnt_root}")
        print(f"DRY-RUN: mount {disk}2 {mnt_root}/boot")
        print(f"DRY-RUN: mount {disk}1 {mnt_root}/boot/efi")
        return True

    mnt_root.mkdir(parents=True, exist_ok=True)
    # Best-effort clean slate
    subprocess.run(["umount", "-R", str(mnt_root)], check=False)

    # Mount btrfs root subvol
    try:
        sh(["mount", "-o", "subvol=root", "LABEL=FEDORA", str(mnt_root)], check=True)
    except subprocess.CalledProcessError:
        # Fallback: mount top-level then subvol=root by path (rare)
        sh(["mount", "LABEL=FEDORA", str(mnt_root)], check=True)

    (mnt_root / "boot").mkdir(parents=True, exist_ok=True)
    (mnt_root / "boot/efi").mkdir(parents=True, exist_ok=True)

    # Mount /boot and EFI if present
    subprocess.run(["mount", f"{disk}2", str(mnt_root / "boot")], check=False)
    subprocess.run(["mount", f"{disk}1", str(mnt_root / "boot/efi")], check=False)

    return True


def need_root():
    if os.geteuid() != 0:
        die("Run as root: sudo python3 mash_full_loop.py <image.raw> ...")

def stage_bootstrap(mountpoint: Path, dry_run: bool = False):
    """Stage the Dojo + bootstrap assets onto the DATA partition.

    Layout on the new system:
      /data/mash-staging/        -> staged installer + Dojo bundle
      /data/mash-logs/           -> logs from first boot units
    """
    dst = mountpoint / "mash-staging"
    logs = mountpoint / "mash-logs"
    banner(f"Staging Dojo/bootstrap into {dst}")
    if dry_run:
        print(f"DRY-RUN: mkdir -p {dst} {logs}")
        print("DRY-RUN: copy dojo_bundle/* + mash_forge.py + helpers/ + wheel/ into mash-staging")
        return

    dst.mkdir(parents=True, exist_ok=True)
    logs.mkdir(parents=True, exist_ok=True)

    # 1) Dojo bundle (preferred entrypoint on first boots)
    dojo_src = BOOTSTRAP_SRC / "dojo_bundle"
    if dojo_src.exists() and dojo_src.is_dir():
        for item in dojo_src.iterdir():
            target = dst / item.name
            if target.exists():
                if target.is_dir():
                    shutil.rmtree(target)
                else:
                    target.unlink()
            if item.is_dir():
                shutil.copytree(item, target)
            else:
                shutil.copy2(item, target)

    # 2) Python forge + helpers (manual / later use)
    for name in ["mash_forge.py", "helpers", "wheel"]:
        src = BOOTSTRAP_SRC / name
        if not src.exists():
            continue
        if src.is_dir():
            target = dst / name
            if target.exists():
                shutil.rmtree(target)
            shutil.copytree(src, target)
        else:
            shutil.copy2(src, dst / name)

    # exec bits
    for p in [dst/"mash_forge.py", dst/"install_dojo.sh"]:
        try:
            if p.exists():
                p.chmod(0o755)
        except Exception:
            pass
    for p in dst.rglob("*.sh"):
        try:
            p.chmod(0o755)
        except Exception:
            pass

    sh(["sync"], check=False)
    print("✅ Dojo/bootstrap staged.")


def offline_locale_patch(root_mnt: Path, dry_run: bool = False):
    banner("Offline locale defaults (en_GB + gb)")
    etc = root_mnt / "etc"
    if not etc.exists():
        print(f"⚠️  Can't find {etc}; skipping.")
        return
    locale_conf = etc / "locale.conf"
    vconsole_conf = etc / "vconsole.conf"
    if dry_run:
        print(f"DRY-RUN: write {locale_conf}: LANG={LOCALE_LANG}")
        print(f"DRY-RUN: write {vconsole_conf}: KEYMAP={VCONSOLE_KEYMAP}")
        return
    locale_conf.write_text(f"LANG={LOCALE_LANG}\n", encoding="utf-8")
    existing = vconsole_conf.read_text(encoding="utf-8") if vconsole_conf.exists() else ""
    lines = [ln for ln in existing.splitlines() if not ln.startswith("KEYMAP=")]
    lines.append(f"KEYMAP={VCONSOLE_KEYMAP}")
    vconsole_conf.write_text("\n".join(lines) + "\n", encoding="utf-8")
    sh(["sync"], check=False)
    print("✅ Offline locale patched.")

def install_firstboot_unit(root_mnt: Path, dry_run: bool = False):
    """Install offline boot units that MUST be available immediately.

    v15 philosophy:
      - Dojo is installed OFFLINE during flashing (no reboot counting games).
      - First boot should be as boring as possible.
      - Early SSH should come up as soon as networking is online.
    """
    banner("Installing offline boot units (early SSH + internet wait)")

    units_dir = root_mnt / "etc/systemd/system"
    wants_dir = root_mnt / "etc/systemd/system/multi-user.target.wants"

    units_dir.mkdir(parents=True, exist_ok=True)
    wants_dir.mkdir(parents=True, exist_ok=True)

    # Copy units/scripts from the staged dojo bundle if present
    bundle_systemd = BOOTSTRAP_SRC / "dojo_bundle" / "systemd"
    bundle_lib = root_mnt / "usr/local/lib/mash/system"
    if dry_run:
        print(f"DRY-RUN: install early-ssh + internet-wait from {bundle_systemd}")
        return

    bundle_lib.mkdir(parents=True, exist_ok=True)

    def _install_unit(name: str):
        src = bundle_systemd / name
        if src.exists():
            shutil.copy2(src, units_dir / name)
            # Enable by symlink
            link = wants_dir / name
            try:
                if link.exists() or link.is_symlink():
                    link.unlink()
                link.symlink_to(Path("..") / name)
            except Exception:
                pass

    # Units
    _install_unit("mash-early-ssh.service")
    _install_unit("mash-internet-wait.service")

    # Scripts
    for script_name in ["early-ssh.sh", "internet-wait.sh"]:
        src = bundle_systemd / script_name
        if src.exists():
            dst = bundle_lib / script_name
            shutil.copy2(src, dst)
            try:
                dst.chmod(0o755)
            except Exception:
                pass

    sh(["sync"], check=False)
    print("✅ Offline boot units installed.")


def install_dojo_offline(root_mnt: Path, dry_run: bool = False, no_nerd_fonts: bool = False, no_emoji_fonts: bool = False):
    """Install Dojo files directly into the target root (no firstboot/reboot counters)."""
    banner("Installing Dojo offline (no boot counters) 🥋")

    bundle = BOOTSTRAP_SRC / "dojo_bundle"
    if not bundle.exists():
        print(f"⚠️ dojo_bundle not found at {bundle} — skipping")
        return

    if dry_run:
        print(f"DRY-RUN: would install dojo from {bundle} into {root_mnt}")
        return

    # Paths in target root
    usr_local_bin = root_mnt / "usr/local/bin"
    usr_local_lib_dojo = root_mnt / "usr/local/lib/mash/dojo"
    usr_local_lib_sys = root_mnt / "usr/local/lib/mash/system"
    xdg_autostart = root_mnt / "etc/xdg/autostart"
    assets_dir = usr_local_lib_dojo / "assets"

    usr_local_bin.mkdir(parents=True, exist_ok=True)
    usr_local_lib_dojo.mkdir(parents=True, exist_ok=True)
    usr_local_lib_sys.mkdir(parents=True, exist_ok=True)
    xdg_autostart.mkdir(parents=True, exist_ok=True)
    assets_dir.mkdir(parents=True, exist_ok=True)

    # Copy trees
    rsync_tree(bundle / "usr_local_lib_mash/dojo", usr_local_lib_dojo)
    rsync_tree(bundle / "usr_local_lib_mash/system", usr_local_lib_sys)

    # Launcher
    shutil.copy2(bundle / "usr_local_bin/mash-dojo-launch", usr_local_bin / "mash-dojo-launch")
    try:
        (usr_local_bin / "mash-dojo-launch").chmod(0o755)
    except Exception:
        pass

    # Autostart desktop file
    shutil.copy2(bundle / "autostart/mash-dojo.desktop", xdg_autostart / "mash-dojo.desktop")

    # Assets
    starship_src = bundle / "assets/starship.toml"
    if starship_src.exists():
        shutil.copy2(starship_src, assets_dir / "starship.toml")

    sh(["sync"], check=False)
    print("✅ Dojo installed offline.")




def main():
    need_root()
    ap = argparse.ArgumentParser()
    ap.add_argument("image", help="Fedora *.raw image to flash")
    ap.add_argument("--disk", default="/dev/sda", help="Target disk (MASH)")
    ap.add_argument("--uefi-dir", default="./rpi4uefi", help="PFTF UEFI dir (must contain RPI_EFI.fd)")
    ap.add_argument("--root-end", default="1800GiB", help="End of ROOT partition (p3). p4 uses the rest.")
    ap.add_argument("--efi-size", default="1024MiB")
    ap.add_argument("--boot-size", default="2048MiB")
    ap.add_argument("--no-dracut", action="store_true", help="Pass through to flasher")
    ap.add_argument("--dry-run", action="store_true")
    ap.add_argument("--skip-bootstrap-stage", action="store_true")
    ap.add_argument("--skip-offline-locale", action="store_true")
    ap.add_argument("--auto-firstboot", action="store_true", help="Enable automatic bootstrap on early boot (can interfere with first-boot setup). Default: OFF")
    ap.add_argument("--firstboot-user", default=DEFAULT_FIRSTBOOT_USER, help="User to target for QoL (KDE configs, zsh, etc.)")
    args = ap.parse_args()

    if not NINJA.exists():
        die(f"Missing flasher: {NINJA}")
    img = Path(args.image).resolve()
    if not img.exists():
        die(f"Image not found: {img}")

    banner("FULL LOOP: Flashing Fedora to MASH (MBR 4-part)")
    ninja_cmd = [
        sys.executable, str(NINJA), str(img),
        "--disk", args.disk,
        "--uefi-dir", str(Path(args.uefi_dir).resolve()),
        "--efi-size", args.efi_size,
        "--boot-size", args.boot_size,
        "--root-end", args.root_end,
    ]
    if args.no_dracut:
        ninja_cmd.append("--no-dracut")

    if args.dry_run:
        print("DRY-RUN:", " ".join(ninja_cmd))
    else:
        sh(ninja_cmd, check=True)

    # Stage bootstrap to DATA
    if not args.skip_bootstrap_stage:
        mnt_data = Path("/mnt/mash_data_stage")
        banner("Mounting DATA partition (LABEL=DATA)")
        if mount_by_label("DATA", mnt_data, dry_run=args.dry_run):
            stage_bootstrap(mnt_data, dry_run=args.dry_run)
            if not args.dry_run:
                sh(["umount", str(mnt_data)], check=False)
        else:
            print("⚠️  Could not mount LABEL=DATA; skipping bootstrap staging.")

    # Offline edits to target root
    mnt_root = None
    if (not args.skip_offline_locale) or (args.auto_firstboot):
        mnt_root = Path("/mnt/mash_root_stage")
        banner("Mounting ROOT subvol for offline edits (LABEL=FEDORA, subvol=root)")
        if not mount_root_subvol(mnt_root, dry_run=args.dry_run):
            print("⚠️  Could not mount root subvol; skipping offline edits.")
            mnt_root = None

    if mnt_root and (not args.skip_offline_locale):
        offline_locale_patch(mnt_root, dry_run=args.dry_run)

    if mnt_root and args.auto_firstboot:
        install_dojo_offline(mnt_root, dry_run=args.dry_run, no_nerd_fonts=args.no_nerd_fonts, no_emoji_fonts=args.no_emoji_fonts)
        install_firstboot_unit(mnt_root, dry_run=args.dry_run)

    if mnt_root and (not args.dry_run):
        sh(["umount", str(mnt_root)], check=False)

    banner("FORGE COMPLETE")
    print("✅ Full-loop flash done.")
    print("✅ Bootstrap staged to: /data/mash-staging (on the new drive)")
    if args.auto_firstboot:
        print("✅ Auto-firstboot unit enabled: mash-firstboot.service (runs once on first boot)")
        print("\nIf you ever need to re-run bootstrap manually:")
    print("  sudo /data/mash-staging/mash_forge.py firstboot --argon-one --zsh-starship --screensaver-nuke --with-starship-fallback")

if __name__ == "__main__":
    main()


