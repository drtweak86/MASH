# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

MASH (Minimal, Automated, Self-Hosting) is a Rust-based installer that automates Fedora KDE installation on Raspberry Pi 4B with UEFI boot support. The installer is **destructive by design** but requires explicit user confirmation for dangerous operations.

## Build Commands

```bash
make build-cli          # Build release binary (output: mash-installer/target/release/mash-installer)
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

1. **TUI Mode** (default, no subcommand): Interactive wizard via `tui::run()`
2. **CLI Mode** (subcommands: `preflight`, `flash`): For scripting/automation

### TUI State Machine (`tui/app.rs`)

The TUI progresses through `InstallStep` enum states:
- Welcome → DiskSelection → DownloadSourceSelection → ImageSelection → UefiDirectory → LocaleSelection → Options → Confirmation → Downloading/Flashing → Complete

`FlashConfig` struct accumulates all user choices across screens.

### Key Modules

- `main.rs`: Entry point, mode dispatch, thread spawning for downloads/flashing
- `tui/app.rs`: State machine logic, input handling, state transitions
- `tui/ui.rs`: Ratatui rendering for each screen
- `flash.rs`: Core installation pipeline (partition, format, rsync, UEFI setup)
- `download.rs`: Fedora image and UEFI firmware downloads with progress
- `locale.rs`: Offline locale/keymap patching (has unit tests)

### Installation Pipeline (`flash.rs`)

1. Validate disk and image
2. Loop-mount source image partitions
3. Partition target disk (MBR or GPT, user's choice)
4. Format partitions (FAT32 for EFI, ext4/btrfs for root)
5. rsync filesystems from source to target
6. Configure UEFI boot (`EFI/BOOT/BOOTAA64.EFI`)
7. Apply locale patches
8. Cleanup mounts and loop devices

### Progress Communication

Downloads and flashing run in separate threads, sending updates via `mpsc::Sender` to keep the TUI responsive.

## Core Principles (from docs/DOJO.md)

- **Destructive actions must be explicit**: Always require confirmation
- **GPT/MBR user choice is mandatory**: Never remove this decision from users
- **Scripts should be noisy, clear, and defensive**: No silent failures
- **Overwrites must create a `bak/` mirror**

## External Dependencies

The installer shells out to system commands:
- `lsblk`, `parted`, `mkfs.*`, `mount`/`umount`, `rsync`, `xz`

## Testing Notes

- Always use `--dry-run` flag when testing installation logic
- Unit tests exist for locale patching in `locale.rs`
- Full installation testing requires physical media (too dangerous to automate)
