#!/usr/bin/env python3
"""
MASH Forge Orchestrator (helpers-based)
- Minimal Python glue.
- Real work happens in helpers/ (bash) and optional wheel modules.
"""
import argparse, os, subprocess, sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent
H = ROOT / "helpers"

def sh(cmd, check=True, env=None):
    if isinstance(cmd, str):
        return subprocess.run(cmd, shell=True, check=check, env=env)
    return subprocess.run(cmd, check=check, env=env)

def banner(msg):
    print("\n" + "="*80)
    print(msg)
    print("="*80)

def need_root():
    if os.geteuid() != 0:
        sys.exit("Run as root (sudo).")

def run_helper(name, *args, check=True):
    p = H / name
    if not p.exists():
        raise SystemExit(f"Missing helper: {p}")
    sh([str(p), *args], check=check)

def offline(args):
    need_root()
    banner("OFFLINE STAGE")
    # NOTE: This orchestrator does not re-implement the full flasher.
    # Hook in your proven flasher here (ninja-mbr4-v2.py / holy-loop-fedora-ninja.py),
    # then call the helpers below.
    if args.write_config:
        run_helper("00_write_config_txt.sh", args.efi_mount)
    if args.stage_bootstrap:
        run_helper("01_stage_bootstrap.sh", args.data_mount, str(ROOT))
    banner("Offline stage done")

def firstboot(args):
    need_root()
    banner("FIRSTBOOT STAGE")
    run_helper("10_locale_uk.sh")
    run_helper("11_snapper_init.sh", args.user)
    run_helper("12_firewall_sane.sh")
    run_helper("13_packages_core.sh")
    run_helper("14_packages_dev.sh")
    run_helper("15_packages_desktop.sh")
    run_helper("16_mount_data.sh", args.user)
    if args.argon_one:
        run_helper("20_argon_one.sh", args.user)
    if args.zsh_starship:
        run_helper("21_zsh_starship.sh", args.user)
    if args.screensaver_nuke:
        run_helper("22_kde_screensaver_nuke.sh", args.user)
    banner("DONE")

def main():
    ap = argparse.ArgumentParser()
    sub = ap.add_subparsers(dest="mode", required=True)

    a = sub.add_parser("offline")
    a.add_argument("--efi-mount", default="/boot/efi", help="Where EFI is mounted")
    a.add_argument("--data-mount", default="/mnt/data", help="Where DATA partition is mounted during offline stage")
    a.add_argument("--write-config", action="store_true")
    a.add_argument("--stage-bootstrap", action="store_true")
    a.set_defaults(func=offline)

    b = sub.add_parser("firstboot")
    b.add_argument("--user", default="drtweak")
    b.add_argument("--argon-one", action="store_true")
    b.add_argument("--zsh-starship", action="store_true")
    b.add_argument("--screensaver-nuke", action="store_true")
    b.set_defaults(func=firstboot)

    args = ap.parse_args()
    args.func(args)

if __name__ == "__main__":
    main()
