# MASH Architecture 🧠

This document describes the technical design and module structure of the MASH installer.

---

## Overview

MASH is a Rust CLI application built with:
- **Clap** for argument parsing
- **Ratatui** for the terminal UI
- **Crossterm** for terminal control
- **Reqwest** for HTTP downloads

The installer operates by:
1. Downloading or locating a Fedora `.raw` image
2. Loop-mounting the source image partitions
3. Partitioning and formatting the target disk
4. Copying filesystems via `rsync`
5. Configuring UEFI boot
6. Running Rust-native post-install stages

---

## Execution Modes

### TUI Mode (Default)

When run without subcommands, MASH launches an interactive terminal wizard:

```bash
sudo mash
```

The TUI uses Ratatui to render a single-screen interface showing:
- Installation steps with status indicators
- Progress bar
- Status messages

**Entry point:** `tui::run()` → `new_app::App` + `new_ui::draw()`

### TUI Evolution: Phase B1 → B3

The TUI has been built in phases to keep CI green while progressively replacing stub data with real, read-only sources.

#### Phase B1 - Stub-backed UI State (no real disk logic)

What changed and why:
- Goal: get a complete wizard flow on-screen quickly, with selectable options everywhere and no blank screens.
- Reason: enable UI iteration without any dependency on disk scanning, downloads, or flashing.

How it was implemented:
- Added a single-screen wizard state machine in `tui/new_app.rs` with explicit step types.
- Built full step rendering in `tui/new_ui.rs` using in-memory stub lists.
- Added progress scaffolding in `tui/progress.rs` to render telemetry without depending on real operations.
- Kept any flashing logic untouched; UI flow driven entirely by stub state.

Result:
- Every step renders a selectable list and supports forward/back navigation.
- Confirmation screen is derived from the current in-memory state.

#### Phase B2 - TUI Flow Completion (stub-safe)

What changed and why:
- Goal: ensure every wizard step is reachable in sequence and selectable.
- Reason: eliminate dead ends and placeholder content before introducing real data sources.

How it was implemented:
- Rewired step transitions so `PartitionLayout -> PartitionCustomize -> DownloadSourceSelection`.
- Added explicit per-step input handling for download steps, still stubbed.
- Replaced "content not loaded yet" placeholders with real, selectable stub options.
- Expanded confirmation summary to include more selected values.

Result:
- The entire wizard path is traversable from Welcome to Complete using only stub state.
- Download steps are simulated but selectable, with no side effects.

#### Phase B3 - Read-only data plumbing behind feature flags

What changed and why:
- Goal: start plumbing real data sources while keeping behavior non-destructive by default.
- Reason: allow real-world validation of lists without any disk writes or downloads.

How it was implemented:
- Added `tui/data_sources.rs` for read-only data collectors (no side effects).
- Introduced feature flags (environment variables) to enable real data per category:
  - `MASH_TUI_REAL_DATA=1` enables all read-only sources.
  - `MASH_TUI_REAL_DISKS=1` enables disk scan from `/sys/block`.
  - `MASH_TUI_REAL_IMAGES=1` enables local image metadata scan + remote metadata list.
  - `MASH_TUI_REAL_LOCALES=1` enables locale/keymap lists from system files.
  - `MASH_TUI_IMAGE_DIRS=/path1:/path2` overrides local image scan paths.
- Disk scan uses `/sys/block` only and formats sizes; no writes.
- Image metadata uses filenames for local matches and enum-driven remote metadata; no downloads.
- Locale/keymap uses `/usr/share/i18n/SUPPORTED` and `/usr/share/X11/xkb/rules/base.lst`.
- Confirmation summary now reflects real selections (when flags are enabled) without altering UI layout.

Result:
- Default behavior remains stubbed and safe.
- Real lists can be turned on for validation without any destructive operations.

### CLI Mode

For scripting and automation, use the `flash` subcommand:

```bash
sudo mash flash --disk /dev/sda --scheme mbr --download-image --download-uefi --yes-i-know
```

The `preflight` subcommand runs system checks:

```bash
sudo mash preflight
```

**Entry point:** `main.rs` dispatches based on `cli::Command`

---

## Module Structure

```
src/
├── main.rs           # Entry point, mode dispatch
├── cli.rs            # CLI argument definitions
├── disk_ops.rs       # Rust-native disk ops (dry-run safe)
├── flash.rs          # Installation pipeline
├── download.rs       # Image and UEFI downloads
├── locale.rs         # Locale configuration
├── preflight.rs      # System checks
├── errors.rs         # Error types
├── logging.rs        # Log setup
├── stages/           # Rust-native post-install stages
└── tui/
    ├── mod.rs        # Terminal setup, main loop
    ├── new_app.rs    # App state, InstallStep, ProgressEvent
    ├── new_ui.rs     # UI rendering
    ├── flash_config.rs # FlashConfig struct
    ├── progress.rs   # Phase tracking
    ├── input.rs      # Text input widget
    ├── widgets.rs    # Reusable components
    ├── app.rs        # Legacy wizard (gated)
    └── ui.rs         # Legacy UI (gated)
```

---

## Installation Pipeline

The core installation logic lives in `flash.rs`. Here's the high-level flow:

### 1. Setup Phase
- Create work directory with mount points
- Attach source image to loop device (`losetup`)
- Detect btrfs subvolumes in source

### 2. Mount Source
- Mount source EFI partition (FAT32)
- Mount source BOOT partition (ext4)
- Mount source ROOT partition (btrfs with subvolumes: root, home, var)

### 3. Partition Target
- Create partition table (MBR or GPT based on user choice)
- Create 4 partitions:
  - **p1 (EFI):** 1 GiB, FAT32, esp flag
  - **p2 (BOOT):** 2 GiB, ext4
  - **p3 (ROOT):** ~1.8 TiB, btrfs
  - **p4 (DATA):** Remaining space, ext4

### 4. Format Target
- `mkfs.vfat` for EFI
- `mkfs.ext4` for BOOT and DATA
- `mkfs.btrfs` for ROOT, then create subvolumes

### 5. Copy Filesystems
- `rsync -aHAXx` from source to target for each partition
- Preserves permissions, ACLs, xattrs, and hard links

### 6. Configure UEFI
- Copy UEFI firmware files to EFI partition
- Ensure `EFI/BOOT/BOOTAA64.EFI` exists
- Configure GRUB if needed

### 7. Apply Locale
- Patch `/etc/locale.conf` and `/etc/vconsole.conf`
- Configure keyboard layout

### 8. Cleanup
- Unmount all partitions (target then source)
- Detach loop device
- Remove work directory

After the flash pipeline completes, optional post-install stages can be invoked via the `--stage` flag. See `docs/INSTALL_STAGES_MODULE.md` for the full catalog and usage.

---

## Rust-Native Disk Operations

`mash_installer::disk_ops` provides Rust-native, dry-run-safe scaffolding for disk probing, partition planning, formatting, mounting, and verification. Real disk mutations are intentionally gated until the implementation is fully ported. See `docs/DISK_OPS_MODULE.md` for details on API behavior and dry-run semantics.

---

## Partition Scheme: MBR vs GPT

### Why Both?

Raspberry Pi UEFI firmware behavior varies by:
- Firmware version
- Boot media type (SD vs USB vs NVMe)
- Specific Pi 4 revision

**MBR (msdos)** is the default because:
- Maximum compatibility with all UEFI firmware versions
- Simpler partition table structure
- Reliable on all boot media

**GPT** is available for:
- Users who specifically want it
- Large disks (>2 TiB)
- Modern setups with known-good firmware

### User Choice is Mandatory

MASH never automatically selects between MBR and GPT. The user must explicitly choose via:
- TUI selection screen
- `--scheme mbr` or `--scheme gpt` flag

This is a core design principle — see [DOJO.md](DOJO.md).

---

## Progress Communication

The flash operation runs in a background thread to keep the UI responsive. Communication happens via `mpsc` channels:

```rust
// In flash.rs
ctx.send_progress(ProgressUpdate::PhaseStarted(Phase::Partitioning));
// ... do work ...
ctx.send_progress(ProgressUpdate::PhaseCompleted(Phase::Partitioning));

// In TUI
while let Ok(update) = progress_rx.try_recv() {
    match update {
        ProgressUpdate::PhaseStarted(phase) => { /* update UI */ }
        ProgressUpdate::PhaseCompleted(phase) => { /* update UI */ }
        ProgressUpdate::Status(msg) => { /* show message */ }
    }
}
```

### Phases

1. `Validation` — Check paths and permissions
2. `Mounting` — Mount source image
3. `Partitioning` — Create partition table
4. `Formatting` — Format filesystems
5. `CopyingEfi` — Copy EFI partition
6. `CopyingBoot` — Copy boot partition
7. `CopyingRoot` — Copy root filesystem
8. `ConfiguringUefi` — Set up boot
9. `ApplyingLocale` — Configure locale
10. `Cleanup` — Unmount and detach

---

## Download Module

`download.rs` handles:

### Fedora Image Download
- Constructs URL based on version and edition
- Downloads `.raw.xz` with progress indication
- Decompresses to `.raw`

### UEFI Firmware Download
- Fetches latest release from `pftf/RPi4` GitHub repo
- Downloads and extracts firmware zip
- Places files in specified directory

---

## Locale Configuration

`locale.rs` provides offline locale patching:

- Modifies `/etc/locale.conf` for language
- Modifies `/etc/vconsole.conf` for keyboard layout
- Runs at install time, not first boot
- Has unit tests for validation

---

## Error Handling

- Uses `anyhow` for error context chaining
- Custom `MashError` enum in `errors.rs`
- All destructive operations wrapped in cleanup guards
- Cleanup runs even on error (via `CleanupGuard` struct)

---

## External Commands

MASH shells out to these system utilities:

| Command | Purpose |
|---------|---------|
| `lsblk` | List block devices |
| `parted` | Partition disk |
| `mkfs.vfat` | Format FAT32 |
| `mkfs.ext4` | Format ext4 |
| `mkfs.btrfs` | Format btrfs |
| `btrfs subvolume` | Create btrfs subvolumes |
| `mount` / `umount` | Mount filesystems |
| `rsync` | Copy files |
| `losetup` | Loop device management |
| `xz` | Decompress images |

These must be available on the host system.

---

## Legacy Code

The `tui/app.rs` and `tui/ui.rs` modules contain a previous multi-screen wizard implementation. These are:
- Gated with `#![allow(dead_code)]`
- Not the default entry point
- Preserved for reference

The current default uses `tui/new_app.rs` and `tui/new_ui.rs`.

---

## Future Considerations

> **Note:** These are architectural observations, not planned features.

- **Btrfs subvolumes:** Currently hardcoded (root, home, var). Could be configurable.
- **Partition sizes:** Configurable via CLI but not TUI. TUI could expose these.
- **First-boot hooks:** Infrastructure exists but hooks are minimal.

---

## See Also

- [QUICKSTART.md](QUICKSTART.md) — User guide
- [DOJO.md](DOJO.md) — Development principles
- [DEPLOYMENT.md](DEPLOYMENT.md) — Building and packaging
