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
├── flash.rs          # Installation pipeline
├── download.rs       # Image and UEFI downloads
├── locale.rs         # Locale configuration
├── preflight.rs      # System checks
├── errors.rs         # Error types
├── logging.rs        # Log setup
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
