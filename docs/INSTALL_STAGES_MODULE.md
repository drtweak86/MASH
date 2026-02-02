# Installation Stages Module (mash_installer::stages)

This document describes the Rust-native installation stages executed after the core flash pipeline. Stages are invoked explicitly via the CLI `--stage` flag and run as best-effort system configuration steps.

---

## Invocation

Stages are dispatched by name or numeric prefix:

```bash
sudo mash --stage 00
sudo mash --stage 00_write_config_txt --stage-arg /boot/efi
```

Stage arguments are passed via `--stage-arg` and interpreted per stage. If a required argument is missing, the stage returns an error.

---

## Stage Catalog

Each stage returns `Result<()>` and logs to stdout/stderr. When external commands fail, most stages log a warning and continue unless the stage explicitly requires the command to succeed.

| Stage | Purpose | Required Args | Notes / Output |
|------:|---------|---------------|----------------|
| 00_write_config_txt | Writes a safe `config.txt` for Pi4 UEFI | `EFI mount path` (default `/boot/efi`) | Writes `config.txt` and runs `sync`. |
| 01_stage_bootstrap | Stages the bootstrap payload to the data mount | `data mount`, `mash_helpers root` | Uses `rsync`; marks shell scripts executable; outputs next-step message. |
| 02_early_ssh | Installs offline early-SSH systemd unit + script | `target root` | Writes `/usr/local/lib/mash/early-ssh.sh` and systemd unit. |
| 02_internet_wait | Installs internet-wait systemd unit + script | `target root`, `staging dir` | Copies service + script and symlinks into `multi-user.target.wants`. |
| 03_fail2ban_lite | Best-effort fail2ban install + sshd jail | _none_ | Installs `fail2ban`, writes jail config, enables service. |
| 03_stage_starship_toml | Stages `starship.toml` into assets | `staging dir`, `starship.toml path` | Copies config into `assets/`. |
| 05_fonts_essential | Installs essential fonts | _none_ | Uses `dnf`, downloads JetBrainsMono Nerd Font (best-effort). |
| 10_locale_uk | Sets `en_GB` locale + GB keymap | _none_ | Uses `dnf` + `localectl`, logs warnings on failure. |
| 11_snapper_init | Initializes snapper config for btrfs | `user` (default `DrTweak`) | Installs `snapper`, creates config, adjusts permissions. |
| 12_firewall_sane | Installs offline early-ssh service | `target root`, `staging dir` | Copies service + script and symlinks unit. |
| 13_packages_core | Installs core packages | _none_ | Uses `package_management::install_packages`. |
| 14_packages_dev | Installs dev/build packages | _none_ | Uses `package_management::install_packages`. |
| 15_packages_desktop | Installs desktop/media packages | _none_ | Uses `package_management::install_packages`. |
| 17_brave_browser | Installs Brave (best-effort) | `user` (default `drtweak`) | Configures repo, logs to `/data/mash-logs/brave.log`, falls back to Firefox on aarch64. |
| 17_brave_default | Sets Brave as default browser (best-effort) | `user` (default `drtweak`) | Skips if no internet; logs to `/data/mash-logs/brave.log`. |
| 20_argon_one | Installs Argon One V2 support | `user` (unused) | Clones repo to `/opt/argononed` and runs install script if executable. |
| 21_zsh_starship | Installs Zsh + Starship prompt | `user` (default `DrTweak`) | Installs `zsh`, installs `starship` if missing, updates `.zshrc`. |
| 22_kde_screensaver_nuke | Disables KDE screensaver/DPMS | `user` (default `DrTweak`) | Uses `kwriteconfig5` + `xset` under the user. |

---

## Error Handling

- Stages return an error when required arguments are missing.
- Many stages log warnings and continue if package installs or best-effort commands fail.
- Long-running or online-dependent steps (fonts, browser, argon) are best-effort by design.

---

## Related Documentation

- `docs/DISK_OPS_MODULE.md` for disk operation scaffolding and dry-run semantics.
- `docs/ARCHITECTURE.md` for module structure and pipeline flow.
