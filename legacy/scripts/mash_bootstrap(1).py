#!/usr/bin/env python3
"""
MASH Bootstrap (Fedora) — Master List Installer
==============================================

Goals:
- Category-based, deduped package list.
- Idempotent: skips packages already installed.
- Fedora-native: uses dnf (installs weak deps / "recommended" by default).
- Robust: skips unavailable packages, can enable RPM Fusion (+ tainted), can handle ffmpeg swap.

Run:
  sudo ./mash_bootstrap.py

Common options:
  sudo ./mash_bootstrap.py --with-kodi --with-tainted
  sudo ./mash_bootstrap.py --dry-run
"""

from __future__ import annotations

import argparse
import os
import shlex
import subprocess
import sys
from dataclasses import dataclass
from typing import List, Set


def sh(cmd: List[str], check: bool = True) -> subprocess.CompletedProcess:
    return subprocess.run(cmd, check=check)


def banner(msg: str) -> None:
    print("\n" + "=" * 80)
    print(msg)
    print("=" * 80)


def is_installed_rpm(pkg: str) -> bool:
    return subprocess.call(["rpm", "-q", pkg], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL) == 0


def dedupe_keep_order(items: List[str]) -> List[str]:
    seen: Set[str] = set()
    out: List[str] = []
    for x in items:
        x = x.strip()
        if not x or x.startswith("#"):
            continue
        if x not in seen:
            seen.add(x)
            out.append(x)
    return out


@dataclass
class Category:
    name: str
    pkgs: List[str]


def dnf_install(pkgs: List[str], *, allow_erasing: bool = False, dry_run: bool = False) -> None:
    pkgs = dedupe_keep_order(pkgs)
    to_install = [p for p in pkgs if not is_installed_rpm(p)]
    if not to_install:
        print("✅ Nothing new to install in this step.")
        return

    cmd = [
        "dnf", "install", "-y",
        "--skip-unavailable",
        "--setopt=install_weak_deps=True",
    ]
    if allow_erasing:
        cmd.append("--allowerasing")
    cmd.extend(to_install)

    if dry_run:
        print("DRY-RUN:", " ".join(shlex.quote(c) for c in cmd))
        return

    sh(cmd, check=True)


def dnf_upgrade(*, dry_run: bool = False) -> None:
    cmd = ["dnf", "upgrade", "--refresh", "-y"]
    if dry_run:
        print("DRY-RUN:", " ".join(shlex.quote(c) for c in cmd))
        return
    sh(cmd, check=True)


def ensure_root() -> None:
    if os.geteuid() != 0:
        print("❌ Please run as root (use sudo).")
        sys.exit(1)


def enable_rpmfusion(with_tainted: bool, dry_run: bool) -> None:
    banner("Repos: RPM Fusion (free + nonfree)")
    free_rel = "rpmfusion-free-release"
    nonfree_rel = "rpmfusion-nonfree-release"
    fed = subprocess.check_output(["rpm", "-E", "%fedora"], text=True).strip()

    if is_installed_rpm(free_rel) and is_installed_rpm(nonfree_rel):
        print("✅ RPM Fusion release packages already installed.")
    else:
        urls = [
            f"https://download1.rpmfusion.org/free/fedora/rpmfusion-free-release-{fed}.noarch.rpm",
            f"https://download1.rpmfusion.org/nonfree/fedora/rpmfusion-nonfree-release-{fed}.noarch.rpm",
        ]
        cmd = ["dnf", "install", "-y"] + urls
        if dry_run:
            print("DRY-RUN:", " ".join(shlex.quote(c) for c in cmd))
        else:
            sh(cmd, check=True)

    if with_tainted:
        banner("Repos: RPM Fusion tainted (for libdvdcss etc.)")
        dnf_install(["rpmfusion-free-release-tainted", "rpmfusion-nonfree-release-tainted"], dry_run=dry_run)



def setup_snapper(username: str, dry_run: bool = False) -> None:
    """
    Atomic shield: ensure snapper is installed and initialize snapshots for / early,
    so you can roll back if anything goes sideways mid-bootstrapping.
    """
    banner("Atomic Shield: Snapper for /")
    # Ensure snapper is present first (idempotent)
    dnf_install(["snapper", "snapper-plugins"], dry_run=dry_run)

    if dry_run:
        print("DRY-RUN: snapper -c root create-config /")
        print("DRY-RUN: chmod a+rx /.snapshots")
        print(f"DRY-RUN: chown :{shlex.quote(username)} /.snapshots  (best-effort)")
        return

    # Create config (safe if already exists)
    subprocess.run(["snapper", "-c", "root", "create-config", "/"], check=False)

    # Make snapshots dir browsable and allow your user group to access (best-effort)
    subprocess.run(["chmod", "a+rx", "/.snapshots"], check=False)
    subprocess.run(["chown", f":{username}", "/.snapshots"], check=False)


def setup_uk_locale(dry_run: bool = False) -> None:
    """
    Proper English: force UK locale + keyboard.
    """
    banner("Locale: en_GB + GB keyboard")
    if dry_run:
        print("DRY-RUN: dnf install -y langpacks-en_GB")
        print("DRY-RUN: localectl set-locale LANG=en_GB.UTF-8")
        print("DRY-RUN: localectl set-x11-keymap gb")
        return

    subprocess.run(["dnf", "install", "-y", "langpacks-en_GB"], check=False)
    subprocess.run(["localectl", "set-locale", "LANG=en_GB.UTF-8"], check=False)
    subprocess.run(["localectl", "set-x11-keymap", "gb"], check=False)


def mount_data_partition(username: str, dry_run: bool = False) -> None:
    """
    Ensure the 1.9TiB DATA partition (LABEL=DATA) is mounted at /data and owned by the user.
    """
    banner("Storage: ensure DATA partition mounted at /data")
    fstab_line = "LABEL=DATA  /data  ext4  defaults,noatime  0  2\n"

    if dry_run:
        print("DRY-RUN: mkdir -p /data")
        print("DRY-RUN: append fstab line if missing:", fstab_line.strip())
        print("DRY-RUN: mount -a")
        print(f"DRY-RUN: chown {shlex.quote(username)}:{shlex.quote(username)} /data")
        return

    os.makedirs("/data", exist_ok=True)

    # Append to /etc/fstab only if it's not already present (by mountpoint or label)
    try:
        with open("/etc/fstab", "r", encoding="utf-8") as f:
            fstab = f.read()
    except Exception:
        fstab = ""

    if ("/data" not in fstab) and ("LABEL=DATA" not in fstab):
        with open("/etc/fstab", "a", encoding="utf-8") as f_append:
            f_append.write(fstab_line)

    subprocess.run(["mount", "-a"], check=False)
    subprocess.run(["chown", f"{username}:{username}", "/data"], check=False)


def maybe_switch_ffmpeg(dry_run: bool) -> None:
    banner("Media: Ensure ffmpeg from RPM Fusion (swap from ffmpeg-free if needed)")
    if is_installed_rpm("ffmpeg-free") and not is_installed_rpm("ffmpeg"):
        print("⚠️ Detected ffmpeg-free; switching to RPM Fusion ffmpeg with --allowerasing ...")
        dnf_install(["ffmpeg"], allow_erasing=True, dry_run=dry_run)
    else:
        print("✅ ffmpeg swap not needed.")


def enable_services(dry_run: bool) -> None:
    banner("Services: firewalld + sshd")
    cmds = [
        ["systemctl", "enable", "--now", "firewalld"],
        ["systemctl", "enable", "--now", "sshd"],
    ]
    for cmd in cmds:
        if dry_run:
            print("DRY-RUN:", " ".join(shlex.quote(c) for c in cmd))
        else:
            subprocess.run(cmd, check=False)

    banner("Firewall: allow mosh (best-effort)")
    if dry_run:
        print("DRY-RUN: firewall-cmd --permanent --add-service=mosh && firewall-cmd --reload")
        return
    subprocess.run(["firewall-cmd", "--permanent", "--add-service=mosh"], check=False)
    subprocess.run(["firewall-cmd", "--reload"], check=False)


def print_summary(categories: List[Category]) -> None:
    banner("Summary: Categories & package counts (deduped)")
    all_pkgs: List[str] = []
    for cat in categories:
        pkgs = dedupe_keep_order(cat.pkgs)
        all_pkgs.extend(pkgs)
        print(f"- {cat.name}: {len(pkgs)} pkgs")
    print(f"\nTotal (pre-skip-installed): {len(dedupe_keep_order(all_pkgs))} pkgs")


def main() -> int:
    ensure_root()

    ap = argparse.ArgumentParser(description="MASH Bootstrap (Fedora) — master list installer.")
    ap.add_argument("--user", default="DrTweak", help="Primary username/group for ownership (default: DrTweak)")
    ap.add_argument("--no-upgrade", action="store_true", help="Skip dnf upgrade --refresh")
    ap.add_argument("--no-rpmfusion", action="store_true", help="Do NOT install/enable RPM Fusion repos")
    ap.add_argument("--with-tainted", action="store_true", help="Enable RPM Fusion tainted repos (libdvdcss etc.)")
    ap.add_argument("--with-kodi", action="store_true", help="Install Kodi + key addons")
    ap.add_argument("--with-snapper", action="store_true", help="Install snapper + plugins (does not auto-configure)")
    ap.add_argument("--no-snapper-init", action="store_true", help="Do NOT initialize snapper for / early")
    ap.add_argument("--no-uk-locale", action="store_true", help="Do NOT force en_GB locale + GB keymap")
    ap.add_argument("--no-data-mount", action="store_true", help="Do NOT ensure LABEL=DATA is mounted at /data")
    ap.add_argument("--dry-run", action="store_true", help="Print commands but do not execute")
    args = ap.parse_args()

    categories: List[Category] = []

    categories.append(Category("Core utilities", [
        "borgbackup", "rclone", "rsync", "pv", "zstd", "tar",
        "ncurses", "btrfs-progs",
    ]))

    categories.append(Category("Dev & build", [
        "git", "cmake", "ninja",
        "gcc", "gcc-c++", "ccache",
        "pkgconf", "autoconf", "automake", "libtool",
        "python3-devel", "patchelf",
    ]))

    categories.append(Category("QoL & shell", [
        "htop", "neovim", "tmux",
        "wget", "curl",
        "unzip", "zip", "tree",
        "bat",
        "doublecmd-qt",
        "zsh", "starship",
    ]))

    categories.append(Category("System tools", [
        "btop",
        "btrfs-assistant",
        "nvme-cli",
    ]))

    categories.append(Category("Networking", [
        "openssh-clients", "openssh-server",
        "mosh",
        "firewalld", "firewall-config", "nftables",
        "iproute", "iputils",
        "bind-utils",
        "tcpdump", "nmap", "nmap-ncat",
        "ethtool", "traceroute",
        "net-tools",
        "avahi", "avahi-tools",
        "socat", "whois",
    ]))

    categories.append(Category("Graphics & input libs", [
        "mesa-dri-drivers", "mesa-libEGL", "mesa-libGL",
        "libdrm", "libinput",
        "libxkbcommon", "libxkbcommon-x11",
        "freetype", "harfbuzz", "fribidi",
        "fmt", "spdlog",
        "sqlite", "taglib",
        "tinyxml", "tinyxml2",
        "openssl",
        "pipewire", "pipewire-pulseaudio",
        "alsa-utils",
        "waylandpp",
    ]))

    categories.append(Category("Multimedia codecs", [
        "gstreamer1-plugins-ugly",
        "gstreamer1-plugins-bad-free-extras",
    ]))

    categories.append(Category("Database client libs", [
        "mariadb-connector-c",
    ]))

    if args.with_snapper:
        categories.append(Category("Btrfs snapshots (snapper)", [
            "snapper", "snapper-plugins",
        ]))

    if args.with_kodi:
        categories.append(Category("Media: Kodi stack", [
            "kodi",
            "kodi-inputstream-adaptive",
            "ffmpeg",
            "dav1d",
            "libdvdread",
            "libdvdnav",
            "libdvdcss",
        ]))

    print_summary(categories)

    # --- Scoot Boogie: early safety + comfort ---
    if not args.no_snapper_init:
        setup_snapper(args.user, dry_run=args.dry_run)

    if not args.no_uk_locale:
        setup_uk_locale(dry_run=args.dry_run)

    if not args.no_rpmfusion:
        enable_rpmfusion(with_tainted=args.with_tainted, dry_run=args.dry_run)

    if not args.no_upgrade:
        banner("System update: dnf upgrade --refresh")
        dnf_upgrade(dry_run=args.dry_run)

    for cat in categories:
        banner(f"Install: {cat.name}")
        if cat.name == "Media: Kodi stack":
            maybe_switch_ffmpeg(dry_run=args.dry_run)
        dnf_install(cat.pkgs, dry_run=args.dry_run)


    if not args.no_data_mount:
        mount_data_partition(args.user, dry_run=args.dry_run)
    enable_services(dry_run=args.dry_run)

    banner("DONE")
    print("Oh My Zsh (not an RPM):")
    print("  sh -c \"$(curl -fsSL https://raw.githubusercontent.com/ohmyzsh/ohmyzsh/master/tools/install.sh)\"")
    print("Starship init (zsh):")
    print("  echo 'eval \"$(starship init zsh)\"' >> ~/.zshrc")
    print("If libdvdcss wasn't found: rerun with --with-tainted")

    return 0


if __name__ == "__main__":
    raise SystemExit(main())
