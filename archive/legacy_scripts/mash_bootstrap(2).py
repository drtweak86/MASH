#!/usr/bin/env python3
"""
MASH Bootstrap (Fedora) — Master List Installer
==============================================

Goals:
- Category-based, deduped package list.
- Idempotent: skips packages already installed.
- Fedora-native: uses dnf and keeps weak deps ("recommended") enabled.
- Robust: skips unavailable packages, can enable RPM Fusion (+ tainted), can handle ffmpeg swap.
- Scoot Boogie extras:
  - Initialize Snapper early (atomic shield).
  - Force UK locale + keyboard.
  - Ensure 4TB DATA partition mounts at /data.

Run:
  sudo ./mash_bootstrap.py

Common:
  sudo ./mash_bootstrap.py --dry-run
  sudo ./mash_bootstrap.py --with-kodi --with-tainted
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


def dnf_pkg_available(pkg: str) -> bool:
    """Best-effort check whether *a package name* exists in enabled repos."""
    # dnf5 usually has repoquery; if not, we just return True and let --skip-unavailable handle it.
    try:
        r = subprocess.run(
            ["dnf", "-q", "repoquery", "--latest-limit", "1", pkg],
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
        )
        return r.returncode == 0
    except FileNotFoundError:
        return True
    except Exception:
        return True


def dnf_install(pkgs: List[str], *, allow_erasing: bool = False, dry_run: bool = False) -> None:
    pkgs = dedupe_keep_order(pkgs)

    # Filter already-installed to avoid noisy "already installed" transaction failures on some setups.
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


def setup_snapper(user: str, dry_run: bool = False) -> None:
    """Atomic Shield: install + initialize snapper ASAP."""
    banner("Atomic Shield: Snapper for /")
    # Fedora 43 doesn't ship snapper-plugins; keep it optional if it exists in repos.
    pkgs = ["snapper"]
    if dnf_pkg_available("snapper-plugins"):
        pkgs.append("snapper-plugins")

    dnf_install(pkgs, dry_run=dry_run)

    # If snapper now installed, init config best-effort.
    if not is_installed_rpm("snapper"):
        print("⚠️ snapper not installed; skipping init.")
        return

    if dry_run:
        print("DRY-RUN: snapper -c root create-config /")
        print("DRY-RUN: chmod a+rx /.snapshots")
        print(f"DRY-RUN: chown :{user} /.snapshots  (best-effort)")
        return

    subprocess.run(["snapper", "-c", "root", "create-config", "/"], check=False)
    subprocess.run(["chmod", "a+rx", "/.snapshots"], check=False)
    # This expects a group with the same name as the user; if it doesn't exist, it's harmless.
    subprocess.run(["chown", f":{user}", "/.snapshots"], check=False)


def setup_uk_locale(dry_run: bool = False) -> None:
    banner("Locale: en_GB + GB keyboard")
    if dry_run:
        print("DRY-RUN: dnf install -y langpacks-en_GB")
        print("DRY-RUN: localectl set-locale LANG=en_GB.UTF-8")
        print("DRY-RUN: localectl set-x11-keymap gb")
        return
    subprocess.run(["dnf", "install", "-y", "langpacks-en_GB"], check=False)
    subprocess.run(["localectl", "set-locale", "LANG=en_GB.UTF-8"], check=False)
    subprocess.run(["localectl", "set-x11-keymap", "gb"], check=False)


def mount_data_partition(user: str, dry_run: bool = False) -> None:
    banner("Storage: ensure DATA partition mounted at /data")
    fstab_line = "LABEL=DATA  /data  ext4  defaults,noatime  0  2\n"

    if dry_run:
        print("DRY-RUN: mkdir -p /data")
        print("DRY-RUN: append fstab line if missing: " + fstab_line.strip())
        print("DRY-RUN: mount -a")
        print(f"DRY-RUN: chown {user}:{user} /data")
        return

    os.makedirs("/data", exist_ok=True)
    try:
        with open("/etc/fstab", "r", encoding="utf-8") as f:
            txt = f.read()
    except FileNotFoundError:
        txt = ""

    if "/data" not in txt:
        with open("/etc/fstab", "a", encoding="utf-8") as f:
            f.write(fstab_line)

    subprocess.run(["mount", "-a"], check=False)
    subprocess.run(["chown", f"{user}:{user}", "/data"], check=False)


def install_starship_fallback(dry_run: bool = False) -> None:
    """If the starship RPM doesn't exist, install via upstream script to /usr/local/bin."""
    banner("Starship: fallback install (upstream)")
    cmd = ["bash", "-lc", "curl -fsSL https://starship.rs/install.sh | sh -s -- -y -b /usr/local/bin"]
    if dry_run:
        print("DRY-RUN:", cmd[-1])
        return
    subprocess.run(cmd, check=False)


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
    ap.add_argument("--user", default="DrTweak", help="Username to chown /data and set snapshot dir group (default: DrTweak)")
    ap.add_argument("--no-upgrade", action="store_true", help="Skip dnf upgrade --refresh")
    ap.add_argument("--no-rpmfusion", action="store_true", help="Do NOT install/enable RPM Fusion repos")
    ap.add_argument("--with-tainted", action="store_true", help="Enable RPM Fusion tainted repos (libdvdcss etc.)")
    ap.add_argument("--with-kodi", action="store_true", help="Install Kodi + key addons")
    ap.add_argument("--with-snapper", action="store_true", help="Install snapper (already initialized by default; this just keeps it in the list)")
    ap.add_argument("--no-snapper-init", action="store_true", help="Do NOT initialize snapper early")
    ap.add_argument("--no-uk-locale", action="store_true", help="Do NOT force UK locale/keyboard")
    ap.add_argument("--no-data-mount", action="store_true", help="Do NOT setup /data automount")
    ap.add_argument("--with-starship-fallback", action="store_true", help="If starship RPM missing, install via upstream script")
    ap.add_argument("--dry-run", action="store_true", help="Print commands but do not execute")
    args = ap.parse_args()

    categories: List[Category] = []

    categories.append(Category("Core utilities", [
        "borgbackup", "rclone", "rsync", "pv", "zstd", "tar",
        "ncurses", "btrfs-progs",
    ]))

    categories.append(Category("Dev & build", [
        "git", "cmake", "ninja-build",  # Fedora package name
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
        "zsh",
        "starship",
    ]))

    categories.append(Category("System tools", [
        "btop", "btrfs-assistant", "nvme-cli",
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

    # Fix typo above safely
    # (We don't need a separate "with-snapper" category because snapper is initialized before installs.)
    categories = categories

    print_summary(categories)

    # Early safety + system personalization
    if not args.no_snapper_init:
        setup_snapper(user=args.user, dry_run=args.dry_run)

    if not args.no_uk_locale:
        setup_uk_locale(dry_run=args.dry_run)

    # Repos / upgrade
    if not args.no_rpmfusion:
        enable_rpmfusion(with_tainted=args.with_tainted, dry_run=args.dry_run)

    if not args.no_upgrade:
        banner("System update: dnf upgrade --refresh")
        dnf_upgrade(dry_run=args.dry_run)

    # Install per category
    for cat in categories:
        banner(f"Install: {cat.name}")
        if cat.name == "Media: Kodi stack":
            maybe_switch_ffmpeg(dry_run=args.dry_run)
        dnf_install(cat.pkgs, dry_run=args.dry_run)

        # Handle starship if RPM missing and user wants fallback
        if cat.name == "QoL & shell" and args.with_starship_fallback:
            if not is_installed_rpm("starship") and not dnf_pkg_available("starship"):
                install_starship_fallback(dry_run=args.dry_run)

    # Storage mount
    if not args.no_data_mount:
        mount_data_partition(user=args.user, dry_run=args.dry_run)

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
