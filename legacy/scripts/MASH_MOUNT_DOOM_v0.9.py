#!/usr/bin/env python3
"""
MASH_MOUNT_DOOM v0.9
===================
Unified Fedora Pi 4 Forge (offline + first-boot stages)

See top of file for usage and intent.
"""

import argparse, subprocess, sys, os, shutil
from pathlib import Path

def sh(cmd, check=True):
    if isinstance(cmd, str):
        return subprocess.run(cmd, shell=True, check=check)
    return subprocess.run(cmd, check=check)

def banner(msg):
    print("\n" + "="*80)
    print(msg)
    print("="*80)

DEFAULT_CONFIG_TXT = """
arm_64bit=1
enable_uart=1
enable_gic=1
armstub=RPI_EFI.fd
disable_commandline_tags=2

[pi4]
dtoverlay=upstream-pi4

[all]
"""

def stage_offline(args):
    banner("STAGE A: OFFLINE USB FORGE")
    if os.geteuid() != 0:
        sys.exit("Run as root")

    if args.stage_bootstrap:
        data = Path("/mnt/data/bootstrap")
        data.mkdir(parents=True, exist_ok=True)
        dst = data / Path(__file__).name
        shutil.copy2(__file__, dst)
        os.chmod(dst, 0o755)
        print(f"Bootstrap staged at {dst}")

    efi = Path("/boot/efi")
    if efi.exists():
        (efi / "config.txt").write_text(DEFAULT_CONFIG_TXT)
        print("Wrote safe Pi4 UEFI config.txt")

def locale_uk():
    sh(["dnf","install","-y","langpacks-en_GB"])
    sh(["localectl","set-locale","LANG=en_GB.UTF-8"])
    sh(["localectl","set-x11-keymap","gb"])

def snapper_init(user):
    sh(["dnf","install","-y","snapper"], check=False)
    sh(["snapper","-c","root","create-config","/"], check=False)
    sh(["chmod","a+rx","/.snapshots"], check=False)
    sh(["chown",f":{user}","/.snapshots"], check=False)

def firewall_sane():
    sh(["systemctl","enable","--now","firewalld"])
    sh(["firewall-cmd","--permanent","--add-service=ssh"])
    sh(["firewall-cmd","--permanent","--add-service=mosh"], check=False)
    sh(["firewall-cmd","--reload"])

def packages_core():
    pkgs = [
        "git","rsync","curl","wget","tmux","neovim",
        "btrfs-progs","btop","mosh","nmap","firewall-config"
    ]
    sh(["dnf","install","-y","--skip-unavailable",*pkgs])

def argon_one():
    banner("Argon One V2 (best effort)")
    sh(["dnf","install","-y","git","gcc","make","i2c-tools","libi2c-devel"], check=False)
    if not Path("/opt/argononed").exists():
        sh(["git","clone","https://gitlab.com/DarkElvenAngel/argononed.git","/opt/argononed"], check=False)
    sh(["bash","-lc","cd /opt/argononed && ./install.sh"], check=False)

def starship_zsh(user):
    sh(["dnf","install","-y","zsh"], check=False)
    sh(["bash","-lc","curl -fsSL https://starship.rs/install.sh | sh -s -- -y"], check=False)
    z = Path(f"/home/{user}/.zshrc")
    if z.exists() and "starship init zsh" not in z.read_text():
        z.write_text(z.read_text() + '\n' + 'eval "$(starship init zsh)"\n')

def stage_firstboot(args):
    banner("STAGE B: FIRST BOOT CONFIG")
    locale_uk()
    snapper_init(args.user)
    firewall_sane()
    packages_core()
    if args.argon_one:
        argon_one()
    if args.starship:
        starship_zsh(args.user)
    banner("DONE")

def main():
    ap = argparse.ArgumentParser()
    sub = ap.add_subparsers(dest="mode", required=True)

    off = sub.add_parser("offline")
    off.add_argument("--raw")
    off.add_argument("--disk")
    off.add_argument("--stage-bootstrap", action="store_true")

    fb = sub.add_parser("firstboot")
    fb.add_argument("--user", default="DrTweak")
    fb.add_argument("--argon-one", action="store_true")
    fb.add_argument("--starship", action="store_true")

    args = ap.parse_args()
    if args.mode == "offline":
        stage_offline(args)
    else:
        stage_firstboot(args)

if __name__ == "__main__":
    main()
