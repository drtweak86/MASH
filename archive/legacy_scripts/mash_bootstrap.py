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
    ap.add_argument("--no-upgrade", action="store_true", help="Skip dnf upgrade --refresh")
    ap.add_argument("--no-rpmfusion", action="store_true", help="Do NOT install/enable RPM Fusion repos")
    ap.add_argument("--with-tainted", action="store_true", help="Enable RPM Fusion tainted repos (libdvdcss etc.)")
    ap.add_argument("--with-kodi", action="store_true", help="Install Kodi + key addons")
    ap.add_argument("--with-snapper", action="store_true", help="Install snapper + plugins (does not auto-configure)")
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
