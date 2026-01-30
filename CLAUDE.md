# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

MASH (Minimal, Automated, Self-Hosting) is a Rust-based installer that automates Fedora KDE installation on Raspberry Pi 4B with UEFI boot support. The installer is **destructive by design** but requires explicit user confirmation for dangerous operations.

**Current version:** v1.2.11

## Build Commands

```bash
make build-cli          # Build release binary (output: mash-installer/target/release/mash)
make dev-cli            # Build debug binary (faster compilation)
make test               # Run Cargo tests
make lint               # Run clippy with -D warnings
make format             # Run cargo fmt
make bump-patch         # Bump version via scripts/bump-version.sh
```

Run a single test:
```bash
cd mash-installer && cargo test test_name
```

## Architecture

### Two Execution Modes

1. **TUI Mode** (default, no subcommand): Interactive terminal wizard via `tui::run()`
2. **CLI Mode** (subcommands: `preflight`, `flash`): For scripting and automation

### Module Structure

```
src/
├── main.rs           # Entry point, mode dispatch
├── cli.rs            # Clap argument parsing, PartitionScheme enum
├── flash.rs          # Core installation pipeline
├── download.rs       # Fedora image and UEFI firmware downloads
├── locale.rs         # Offline locale/keymap patching
├── preflight.rs      # System requirement checks
├── errors.rs         # Error types
├── logging.rs        # Log initialization
└── tui/
    ├── mod.rs        # TUI entry point, terminal setup
    ├── new_app.rs    # New app state (InstallStep, ProgressEvent, App)
    ├── new_ui.rs     # New single-screen UI rendering
    ├── flash_config.rs # FlashConfig, ImageSource, ImageEditionOption
    ├── app.rs        # Legacy wizard state machine (gated, not default)
    ├── ui.rs         # Legacy multi-screen rendering (gated, not default)
    ├── input.rs      # Text input widget
    ├── progress.rs   # Progress tracking and phases
    └── widgets.rs    # Reusable UI components
```

### TUI Architecture

**Current default:** The TUI uses `new_app.rs` and `new_ui.rs` for a streamlined single-screen flow.

**Legacy code:** `app.rs` and `ui.rs` contain the previous multi-screen wizard. These modules are gated with `#![allow(dead_code)]` and are not the default entry point.

### Key Types

- `FlashConfig` — Accumulated user configuration (disk, scheme, locale, etc.)
- `ImageSource` — Local file vs download selection
- `PartitionScheme` — MBR or GPT
- `ProgressUpdate` / `Phase` — Progress communication from flash thread

### Installation Pipeline (`flash.rs`)

1. Validate disk and image paths
2. Loop-mount source image partitions (EFI, BOOT, ROOT with btrfs subvols)
3. Partition target disk (MBR or GPT based on user choice)
4. Format partitions (FAT32 for EFI, ext4 for BOOT/DATA, btrfs for ROOT)
5. rsync filesystems from source to target
6. Configure UEFI boot (`EFI/BOOT/BOOTAA64.EFI`)
7. Apply locale patches if configured
8. Cleanup: unmount and detach loop devices

### Progress Communication

Downloads and flashing run in separate threads, sending `ProgressUpdate` messages via `mpsc::Sender` to keep the TUI responsive.

## Core Principles (from docs/DOJO.md)

- **Destructive actions must be explicit** — Always require confirmation
- **GPT/MBR user choice is mandatory** — Never remove this decision from users
- **Scripts should be noisy, clear, and defensive** — No silent failures
- **Overwrites must create a `bak/` mirror** — Preserve previous state

## External Dependencies

The installer shells out to system commands:
- `lsblk` — List block devices
- `parted` — Partition disk
- `mkfs.vfat`, `mkfs.ext4`, `mkfs.btrfs` — Format partitions
- `mount` / `umount` — Mount filesystems
- `rsync` — Copy files
- `xz` — Decompress images
- `losetup` — Loop device management

## CLI Flags Reference

```
mash flash [OPTIONS] --disk <DISK>

Options:
  --image <PATH>           Path to Fedora .raw image
  --disk <DEVICE>          Target disk (e.g., /dev/sda)
  --scheme <mbr|gpt>       Partition scheme (default: mbr)
  --uefi-dir <PATH>        Directory containing UEFI files
  --download-image         Auto-download Fedora image
  --download-uefi          Auto-download UEFI firmware
  --image-version <VER>    Fedora version (default: 43)
  --image-edition <ED>     Fedora edition (default: KDE)
  --auto-unmount           Auto-unmount target partitions
  --yes-i-know             Confirm destructive operation
  --locale <LANG:KEYMAP>   Locale setting (e.g., en_GB.UTF-8:uk)
  --early-ssh              Enable SSH before graphical login
  --efi-size <SIZE>        EFI partition size (default: 1024MiB)
  --boot-size <SIZE>       BOOT partition size (default: 2048MiB)
  --root-end <SIZE>        ROOT partition end (default: 1800GiB)
  --dry-run                Simulate without changes
```

## Testing Notes

- **Always use `--dry-run` flag** when testing installation logic
- Unit tests exist in `locale.rs` for locale patching
- Full installation testing requires physical media (too dangerous to automate)
- Run `make lint` before committing — CI enforces `-D warnings`

## Git Workflow

- `main` branch is the source of truth
- Backup files go in `bak/` directory
- Legacy scripts preserved in `legacy_scripts/HISTORY/`
