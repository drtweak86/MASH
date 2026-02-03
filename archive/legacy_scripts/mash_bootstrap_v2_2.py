#!/usr/bin/env python3
"""
MASH Bootstrap (Fedora) - Master List Installer (v2.2)
=====================================================

What this does:
- (Optional) Enable RPM Fusion (+ tainted if requested)
- Upgrade system
- Install your “master list” packages grouped by category (deduped + skip already-installed)
- Initialize Snapper on / (best-effort)
- Force UK locale + GB keyboard
- Ensure /data (LABEL=DATA) is mounted
- Enable firewalld + sshd, allow mosh
- Starship install with fallback (optional)
- Final user QoL: ensure ~/.zshrc exists, add starship init idempotently, set default shell to zsh,
  and nuke KDE screen lock / DPMS (best-effort)

Run:
  chmod +x mash_bootstrap_v2_2.py
  sudo ./mash_bootstrap_v2_2.py --dry-run --with-starship-fallback
  sudo ./mash_bootstrap_v2_2.py --with-starship-fallback
"""

from __future__ import annotations

import argparse
import os
import pwd
import shlex
import shutil
import subprocess
import sys
from dataclasses import dataclass
from typing import List, Set


def banner(msg: str) -> None:
    print("\n" + "=" * 80)
    print(msg)
    print("=" * 80)


def run(cmd: List[str], *, check: bool = True, dry_run: bool = False, shell_hint: str | None = None) -> None:
    if dry_run:
        if shell_hint:
            print("DRY-RUN:", shell_hint)
        else:
            print("DRY-RUN:", " ".join(shlex.quote(c) for c in cmd))
        return
    subprocess.run(cmd, check=check)


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
    """Best-effort: returns True if repoquery finds the package."""
    try:
        r = subprocess.run(
            ["dnf", "-q", "repoquery", "--latest-limit", "1", pkg],
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
        )
        return r.returncode == 0
    except Exception:
        # If repoquery isn't available for some reason, don't block installs.
        return True


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
    run(cmd, check=True, dry_run=dry_run)


def dnf_upgrade(*, dry_run: bool = False) -> None:
    run(["dnf", "upgrade", "--refresh", "-y"], check=True, dry_run=dry_run)


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
        run(["dnf", "install", "-y", *urls], check=True, dry_run=dry_run)

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
    banner("Atomic Shield: Snapper for /")
    pkgs = ["snapper"]
    # Plugins package name varies across distros/repos; only attempt if it exists.
    if dnf_pkg_available("snapper-plugins"):
        pkgs.append("snapper-plugins")
    dnf_install(pkgs, dry_run=dry_run)

    if not is_installed_rpm("snapper"):
        print("⚠️ snapper not installed; skipping init.")
        return

    run(["snapper", "-c", "root", "create-config", "/"], check=False, dry_run=dry_run)
    run(["chmod", "a+rx", "/.snapshots"], check=False, dry_run=dry_run)
    # Group-readable for your user group (best-effort)
    run(["chown", f":{user}", "/.snapshots"], check=False, dry_run=dry_run)


def setup_uk_locale(dry_run: bool = False) -> None:
    banner("Locale: en_GB + GB keyboard")
    run(["dnf", "install", "-y", "langpacks-en_GB"], check=False, dry_run=dry_run)
    run(["localectl", "set-locale", "LANG=en_GB.UTF-8"], check=False, dry_run=dry_run)
    run(["localectl", "set-x11-keymap", "gb"], check=False, dry_run=dry_run)


def mount_data_partition(user: str, dry_run: bool = False) -> None:
    banner("Storage: ensure DATA partition mounted at /data")
    fstab_line = "LABEL=DATA  /data  ext4  defaults,noatime  0  2\n"

    run(["mkdir", "-p", "/data"], check=False, dry_run=dry_run)

    if dry_run:
        print("DRY-RUN: append fstab line if missing:", fstab_line.strip())
    else:
        try:
            with open("/etc/fstab", "r", encoding="utf-8") as f:
                txt = f.read()
        except FileNotFoundError:
            txt = ""
        if "/data" not in txt:
            with open("/etc/fstab", "a", encoding="utf-8") as f:
                f.write(fstab_line)

    run(["mount", "-a"], check=False, dry_run=dry_run)
    run(["chown", f"{user}:{user}", "/data"], check=False, dry_run=dry_run)

    if dry_run:
        print("DRY-RUN: verify mountpoint: findmnt /data")
    else:
        subprocess.run(["findmnt", "/data"], check=False)


def install_starship(dry_run: bool = False) -> None:
    if is_installed_rpm("starship"):
        print("✅ starship already installed (RPM).")

    if not is_installed_rpm("starship") and dnf_pkg_available("starship"):
        banner("Starship: installing from repos")
        dnf_install(["starship"], dry_run=dry_run)

    if not is_installed_rpm("starship"):
        banner("Starship: installing upstream binary to /usr/local/bin")
        shell = "curl -fsSL https://starship.rs/install.sh | sh -s -- -y -b /usr/local/bin"
        run(["bash", "-lc", shell], check=False, dry_run=dry_run, shell_hint=shell)

        # Best-effort verification
        if dry_run:
            print("DRY-RUN: /usr/local/bin/starship --version")
        else:
            subprocess.run(["/usr/local/bin/starship", "--version"], check=False)


def enable_services(dry_run: bool) -> None:
    banner("Services: firewalld + sshd")
    run(["systemctl", "enable", "--now", "firewalld"], check=False, dry_run=dry_run)
    run(["systemctl", "enable", "--now", "sshd"], check=False, dry_run=dry_run)

    banner("Firewall: allow mosh (best-effort)")
    run(["firewall-cmd", "--permanent", "--add-service=mosh"], check=False, dry_run=dry_run)
    run(["firewall-cmd", "--reload"], check=False, dry_run=dry_run)


def setup_user_qol(target_user: str, dry_run: bool = False) -> None:
    banner(f"Final QoL: Zsh, Starship, and Power for {target_user}")

    try:
        user_info = pwd.getpwnam(target_user)
        home_dir = user_info.pw_dir
    except KeyError:
        print(f"❌ User {target_user} not found. Skipping QoL steps.")
        return

    # 1) Zsh & Starship Setup (Idempotent)
    zshrc_path = os.path.join(home_dir, ".zshrc")
    starship_line = 'eval "$(starship init zsh)"'

    if not dry_run:
        # Ensure .zshrc exists
        if not os.path.exists(zshrc_path):
            with open(zshrc_path, "w", encoding="utf-8") as f:
                f.write("# Zsh Configuration\n")
            run(["chown", f"{target_user}:{target_user}", zshrc_path], check=False, dry_run=False)

        # Check for existing line before adding
        try:
            with open(zshrc_path, "r", encoding="utf-8") as f:
                content = f.read()
        except Exception:
            content = ""

        if starship_line not in content:
            print(f"✅ Adding Starship init to {zshrc_path}")
            with open(zshrc_path, "a", encoding="utf-8") as f:
                f.write(f"\n{starship_line}\n")

        # Change default shell to Zsh for the user (best-effort)
        if shutil.which("chsh") and os.path.exists("/usr/bin/zsh"):
            run(["chsh", "-s", "/usr/bin/zsh", target_user], check=False, dry_run=False)
        else:
            print("⚠️ chsh or /usr/bin/zsh missing; skipping default shell change.")
    else:
        print(f"DRY-RUN: ensure {zshrc_path} exists + append starship init if missing")
        print(f"DRY-RUN: chsh -s /usr/bin/zsh {target_user}")

    # 2) Disable Screensaver & DPMS (KDE CLI tools) (best-effort)
    kwrite = shutil.which("kwriteconfig6") or shutil.which("kwriteconfig5")
    if not kwrite:
        print("⚠️ kwriteconfig(5/6) not found; skipping KDE config tweaks.")
    else:
        kw = os.path.basename(kwrite)
        kde_cmds = [
            f"{kw} --file kscreenlockerrc --group Daemon --key Autolock false",
            f"{kw} --file powerdevilrc --group AC --group SuspendSession --key suspendType 0",
        ]
        # xset only works under X11; still try (harmless if it fails).
        kde_cmds += ["xset s off", "xset -dpms"]

        for cmd in kde_cmds:
            run(["sudo", "-u", target_user, "sh", "-c", cmd], check=False, dry_run=dry_run)


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

    ap = argparse.ArgumentParser(description="MASH Bootstrap (Fedora) - master list installer.")
    ap.add_argument("--user", default="DrTweak", help="Username for QoL steps + /data chown (default: DrTweak)")
    ap.add_argument("--no-upgrade", action="store_true", help="Skip dnf upgrade --refresh")
    ap.add_argument("--no-rpmfusion", action="store_true", help="Do NOT install/enable RPM Fusion repos")
    ap.add_argument("--with-tainted", action="store_true", help="Enable RPM Fusion tainted repos (libdvdcss etc.)")
    ap.add_argument("--with-kodi", action="store_true", help="Install Kodi + key addons")
    ap.add_argument("--no-snapper-init", action="store_true", help="Do NOT initialize snapper early")
    ap.add_argument("--no-uk-locale", action="store_true", help="Do NOT force UK locale/keyboard")
    ap.add_argument("--no-data-mount", action="store_true", help="Do NOT setup /data automount")
    ap.add_argument("--with-starship-fallback", action="store_true", help="Install starship via upstream if RPM missing")
    ap.add_argument("--dry-run", action="store_true", help="Print commands but do not execute")
    args = ap.parse_args()

    categories: List[Category] = []

    categories.append(Category("Core utilities", [
        "borgbackup", "rclone", "rsync", "pv", "zstd", "tar",
        "ncurses", "btrfs-progs",
    ]))

    categories.append(Category("Dev & build", [
        "git", "cmake", "ninja-build",
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
        # NOTE: starship handled specially below
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

    print_summary(categories)

    if not args.no_snapper_init:
        setup_snapper(user=args.user, dry_run=args.dry_run)

    if not args.no_uk_locale:
        setup_uk_locale(dry_run=args.dry_run)

    if not args.no_rpmfusion:
        enable_rpmfusion(with_tainted=args.with_tainted, dry_run=args.dry_run)

    if not args.no_upgrade:
        banner("System update: dnf upgrade --refresh")
        dnf_upgrade(dry_run=args.dry_run)

    # Install categories
    for cat in categories:
        banner(f"Install: {cat.name}")

        if cat.name == "Media: Kodi stack":
            maybe_switch_ffmpeg(dry_run=args.dry_run)

        dnf_install(cat.pkgs, dry_run=args.dry_run)

    # Starship (special handling)
    if args.with_starship_fallback:
        install_starship(dry_run=args.dry_run)
    else:
        # Only attempt via RPM if it exists; otherwise tell you what to do.
        if dnf_pkg_available("starship"):
            banner("Starship: installing from repos")
            dnf_install(["starship"], dry_run=args.dry_run)
        else:
            banner("Starship")
            print("⚠️ starship not found in repos; rerun with --with-starship-fallback")

    if not args.no_data_mount:
        mount_data_partition(user=args.user, dry_run=args.dry_run)

    enable_services(dry_run=args.dry_run)

    # Final QoL hooks (zshrc, starship init, screensaver nuke)
    setup_user_qol(target_user=args.user, dry_run=args.dry_run)

    banner("DONE - SCOOT BOOGIE COMPLETE")
    print(f"✅ 4TB Fedora System configured for {args.user}.")
    print("🚀 Log out and back in (or reboot) to see your new setup!")
    print("\nOh My Zsh (not an RPM):")
    print('  sh -c "$(curl -fsSL https://raw.githubusercontent.com/ohmyzsh/ohmyzsh/master/tools/install.sh)"')
    print("Starship init (zsh):")
    print('  echo \'eval "$(starship init zsh)"\' >> ~/.zshrc')
    print("If libdvdcss wasn't found: rerun with --with-tainted")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
