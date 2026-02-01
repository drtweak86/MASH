# Issue 9 Script Rustification — Inventory (Non-Legacy)

Date: 2026-02-01
Scope: Non-legacy scripts only. Excludes `legacy/` and `legacy_scripts/` entirely.

## Summary Counts
- helpers/*.sh: 19 files (to port into `mash-installer/src/stages/`)
- scripts/*.py: 1 file (to port into `mash-installer` main flow)
- dojo_bundle/**/*.sh: 16 files (to port into `mash-installer` as `dojo` subcommand/modules)
- install.sh: 1 file (replace with wrapper to `mash-installer`)
- Retained shell template: 1 file (`mash-installer/src/dojo_install_template.sh`)

## Port to `mash-installer` stages (helpers/*.sh)
- helpers/00_write_config_txt.sh
- helpers/01_stage_bootstrap.sh
- helpers/02_early_ssh.sh
- helpers/02_internet_wait.sh
- helpers/03_fail2ban_lite.sh
- helpers/03_stage_starship_toml.sh
- helpers/05_fonts_essential.sh
- helpers/10_locale_uk.sh
- helpers/11_snapper_init.sh
- helpers/12_firewall_sane.sh
- helpers/13_packages_core.sh
- helpers/14_packages_dev.sh
- helpers/15_packages_desktop.sh
- helpers/16_mount_data.sh
- helpers/17_brave_browser.sh
- helpers/17_brave_default.sh
- helpers/20_argon_one.sh
- helpers/21_zsh_starship.sh
- helpers/22_kde_screensaver_nuke.sh

## Port to `mash-installer` main flow (scripts/*.py)
- scripts/mash-full-loop.py

## Replace with wrapper
- install.sh (wrapper to `./mash-installer "$@"` per Issue #9)

## Port to `mash-installer` `dojo` subcommand (dojo_bundle/**/*.sh)
- dojo_bundle/install_dojo.sh
- dojo_bundle/systemd/early-ssh.sh
- dojo_bundle/systemd/internet-wait.sh
- dojo_bundle/usr_local_lib_mash/system/bootcount.sh
- dojo_bundle/usr_local_lib_mash/dojo/argon_one.sh
- dojo_bundle/usr_local_lib_mash/dojo/audio.sh
- dojo_bundle/usr_local_lib_mash/dojo/bootstrap.sh
- dojo_bundle/usr_local_lib_mash/dojo/borg.sh
- dojo_bundle/usr_local_lib_mash/dojo/browser.sh
- dojo_bundle/usr_local_lib_mash/dojo/dojo.sh
- dojo_bundle/usr_local_lib_mash/dojo/firewall.sh
- dojo_bundle/usr_local_lib_mash/dojo/graphics.sh
- dojo_bundle/usr_local_lib_mash/dojo/menu.sh
- dojo_bundle/usr_local_lib_mash/dojo/mount_data.sh
- dojo_bundle/usr_local_lib_mash/dojo/rclone.sh
- dojo_bundle/usr_local_lib_mash/dojo/snapper.sh

## Retain
- mash-installer/src/dojo_install_template.sh (explicitly retained by Issue #9)

## Directories slated for deletion after porting
- helpers/
- scripts/
- dojo_bundle/
- reference/ (Issue #9 acceptance criteria)

