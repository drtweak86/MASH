# MASH Architecture 🧠

This document describes the technical design and modular structure of the MASH installer.

---

## Overview

MASH is a Rust application built around a modular architecture:
- **`mash-installer`**: The main executable, a thin orchestration layer.
- **`mash-core`**: Shared types, state, error definitions, and core configuration.
- **`crates/mash-hal`**: The Hardware Abstraction Layer, handling all low-level, "world-touching" system operations (filesystem, block devices, mounting, partitioning). All external commands (e.g., `rsync`, `parted`, `mount`) are replaced by Rust-native implementations here, where feasible, or safely wrapped.
- **`crates/mash-tui`**: Reusable `ratatui` widgets and UI components, implementing the interactive terminal interface.
- **`crates/mash-workflow`**: The stage engine, defining pipeline steps, orchestration, and state management for the installation process.

The installer operates with a focus on safety and determinism:
1.  **Safety First**: Starts in `[SAFE MODE]`, requiring explicit `DESTROY` arming for destructive actions.
2.  **Source Drive Protection**: Automatically identifies and protects the running system's boot media.
3.  **Enforced Workflow**: Guides the user through Distro → Flavour → Disk → Partition → Review stages.
4.  **Rust-native Operations**: All critical system operations are performed via `mash-hal`, eliminating fragile shell script glue.

---

## Execution Modes

MASH operates in two primary modes, always prioritizing safety and an enforced workflow.

### 1. Interactive TUI Mode (Default)

When run without subcommands, MASH launches an interactive terminal Dojo UI:

```bash
sudo mash
```

**Safety Architecture:**
-   **Safe Mode (`[SAFE MODE]` - Green Header):** The installer always starts in this read-only state. All destructive operations are blocked.
-   **Arming Sequence (`DESTROY` prompt):** To proceed with destructive actions, the user must explicitly type `DESTROY` at a modal prompt.
-   **Armed Mode (`[ARMED]` - Red Header):** Once armed, destructive HAL calls are permitted.

**Enforced Workflow:**
The TUI guides the user through a strict sequence of steps to prevent errors and ensure a complete configuration:
1.  **Welcome**
2.  **Distro Selection**: Choose the operating system (e.g., Fedora, Ubuntu, RPi OS, Manjaro). Only fully wired OSes are shown.
3.  **Flavour / Edition Selection**: Context-aware variants for the chosen distro appear.
4.  **Disk Selection**: Select the target installation disk. The **source drive (boot media) is automatically detected and protected**, raising a UI error if selected accidentally.
5.  **Partition Wizard**: Configure the disk layout (Whole Disk with distro-defined layout or Manual Layout). EFI options are shown only if required.
6.  **Review & Confirm**
7.  **Installation Progress**

The TUI uses `crates/mash-tui` for rendering, with `mash-workflow` orchestrating the state transitions and `mash-hal` performing system operations.

### 2. CLI Mode (For Scripting)

For scripting and automation, use the `flash` subcommand. This mode requires explicit flags to perform destructive operations and disable safety features:

```bash
sudo mash flash \
  --disk /dev/sda \
  --scheme gpt \
  --distro fedora \
  --flavour kde \
  --download-image \
  --auto-unmount \
  --armed \
  --yes-i-know
```

The `preflight` subcommand runs system checks, including hardware detection:

```bash
sudo mash preflight
```

**Entry point:** `main.rs` dispatches based on `cli::Command`.

## Module Structure

The MASH project is organized into a Cargo workspace, promoting modularity and clean dependency boundaries:

```
.
├── Cargo.toml                      # Workspace definition
├── mash-installer/                 # The main executable (thin orchestration)
│   └── src/main.rs                 # Initializes workflow and TUI
├── mash-core/                      # Shared types, state, error definitions, config
│   ├── src/cli.rs                  # CLI argument definitions
│   ├── src/errors.rs               # Custom error types (`MASHError`)
│   ├── src/logging.rs              # Log setup (routed to ~/.mash/mash.log)
│   ├── src/state_manager/          # Installation state persistence
│   └── ...                         # Other shared utilities
└── crates/
    ├── mash-hal/                   # Hardware Abstraction Layer
    │   ├── src/lib.rs              # Traits (MountOps, BlockOps, etc.) + LinuxHal/FakeHal implementations
    │   └── ...                     # Rust-native system operation implementations
    ├── mash-tui/                   # Terminal User Interface components
    │   ├── src/lib.rs              # Reusable Ratatui widgets, input handling, progress rendering
    │   └── ...                     # UI logic, layout grid, safety state display
    └── mash-workflow/              # Stage engine and orchestration
        ├── src/lib.rs              # StageRunner, pipeline logic, OS-specific install stages
        └── ...                     # Resumable workflow management
```
Existing modules within `mash-core` (like `flash.rs`, `preflight.rs`, `download.rs`, `locale.rs`) now delegate their core system interactions to `mash-hal` and their workflow logic to `mash-workflow`.


---

## Installation Pipeline

The core installation logic is orchestrated by `mash-workflow` and executed through a series of well-defined stages, leveraging `mash-hal` for all critical system interactions. This ensures determinism, testability, and safety.

Here's the high-level flow:

### 1. Preflight Checks
- Performed by `mash-workflow`.
- Verifies system requirements, hardware compatibility, and essential tools.

### 2. OS/Image Download (if required)
- Orchestrated by `mash-workflow`.
- Uses `mash-core`'s download logic to fetch OS images and potentially UEFI firmware.
- Supports resume and checksum verification.

### 3. Source Image Preparation
- Handled by `mash-hal`.
- Attaches the source OS image to a loop device.
- Detects partitions and filesystems within the image.

### 4. Target Disk Partitioning
- Orchestrated by `mash-workflow`, driven by OS-specific rules.
- Performed by `mash-hal`.
- Creates partition table (MBR or GPT, based on user choice and OS compatibility).
- Creates OS-defined partitions (e.g., EFI, BOOT, ROOT, DATA).

### 5. Target Filesystem Formatting
- Orchestrated by `mash-workflow`.
- Performed by `mash-hal`.
- Formats target partitions (e.g., FAT32 for EFI, ext4 for BOOT, btrfs for ROOT).
- Creates btrfs subvolumes if required.

### 6. Filesystem Copy
- Orchestrated by `mash-workflow`.
- Performed by `mash-hal`, leveraging `rsync` functionality internally where applicable or Rust-native file copying.
- Copies system files from the prepared source image to the target partitions.
- Preserves permissions, ACLs, xattrs, and hard links.

### 7. UEFI Boot Configuration
- Orchestrated by `mash-workflow`.
- Performed by `mash-hal`.
- Copies UEFI firmware files and ensures `EFI/BOOT/BOOTAA64.EFI` is correctly placed.
- Configures bootloader (e.g., GRUB entries).

### 8. System Configuration & Locale
- Orchestrated by `mash-workflow`.
- Performed by `mash-hal`.
- Applies locale settings (language, keyboard layout).
- Performs other OS-specific post-installation configuration.

### 9. Cleanup
- Orchestrated by `mash-workflow`.
- Performed by `mash-hal`.
- Unmounts all target and source partitions (RAII-based `MountGuard`/`MountPoint` ensures atomic cleanup even on error).
- Detaches loop devices.
- Removes temporary work directories.

---

## Partition Scheme: MBR vs GPT

### Why Both?

The choice between MBR and GPT can be influenced by:
-   **OS Compatibility**: Some operating systems have preferred or mandatory partition schemes.
-   **Hardware Compatibility**: UEFI firmware versions, boot media type (SD vs USB vs NVMe), and specific Raspberry Pi 4 revisions can all play a role.
-   **User Preference**: For advanced users with specific needs.

**MBR (msdos)** is often chosen for:
-   Maximum compatibility with older UEFI firmware versions.
-   Simpler partition table structure.
-   Reliability on a wider range of boot media.

**GPT** is typically used for:
-   Modern systems and larger disks (>2 TiB).
-   When explicitly required by the OS or user.

### User Choice, Guided by OS Rules

MASH guides the user's choice and, where necessary, enforces OS-specific partitioning rules. The user will still make a decision via:
-   TUI selection screen (with dynamic options based on selected OS)
-   `--scheme mbr` or `--scheme gpt` flag (CLI mode)

This approach ensures both user control and system compatibility – see [DOJO.md](DOJO.md) for more on core design principles.

---

## Progress Communication

The installation process runs in a background thread within `mash-workflow` to keep the UI responsive. Communication happens via `mpsc` channels, sending `ProgressUpdate` messages from `mash-workflow` (which in turn receives updates from `mash-hal` for low-level operations) back to the TUI.

```rust
// In mash-workflow/src/lib.rs (example)
self.ctx.send_progress(ProgressUpdate::PhaseStarted(Phase::Partitioning));
// ... mash-hal operations ...
self.ctx.send_progress(ProgressUpdate::PhaseCompleted(Phase::Partitioning));

// In mash-tui/src/lib.rs (example)
while let Ok(update) = progress_rx.try_recv() {
    match update {
        ProgressUpdate::PhaseStarted(phase) => { /* update UI */ }
        ProgressUpdate::PhaseCompleted(phase) => { /* update UI */ }
        ProgressUpdate::Status(msg) => { /* show message */ }
    }
}
```

### Phases

Phases are now defined and managed within `mash-workflow`, providing a clear, resumable sequence:
1.  `Validation`
2.  `Download`
3.  `SetupSource`
4.  `Partitioning`
5.  `Formatting`
6.  `CopyingFiles`
7.  `BootConfiguration`
8.  `PostInstallConfig`
9.  `Cleanup`

---

## Download Module

The `download.rs` module (within `mash-core`) handles fetching OS images and related files. Its operations are orchestrated by `mash-workflow` and leverage `mash-hal` for secure file I/O and temporary storage.

### OS Image Download
- Constructs URLs based on OS, variant, and architecture.
- Downloads `.raw.xz` (or similar compressed formats) with progress indication.
- Supports checksum verification and resume capabilities.

### UEFI Firmware Download (if applicable)
- Fetches firmware when required (e.g., Raspberry Pi UEFI).
- Downloads and extracts firmware archives.
- Places files in the correct location via `mash-hal`.

---

## Locale Configuration

The `locale.rs` module provides offline locale patching. These operations are performed via `mash-hal` to ensure safe and robust system configuration:

-   Modifies `/etc/locale.conf` for language settings.
-   Modifies `/etc/vconsole.conf` for keyboard layout settings.
-   Runs at install time, not first boot.
-   Includes unit tests for validation.

---

## Error Handling

MASH employs robust error handling with `anyhow` for context chaining and `mash_core::error::MASHError` for specific, typed error conditions.

-   **`MASHError` enum**: Defines specific error types like `DiskBusy`, `PermissionDenied`, `SafetyLock`, and `ValidationFailed`, enabling precise error reporting and handling.
-   **Cleanup Guards**: All destructive operations (performed via `mash-hal`) are wrapped in RAII-based cleanup guards (`MountGuard`, `MountPoint`). These ensure that even if an error occurs, mounted filesystems are unmounted and temporary resources are released, preventing system state corruption.
-   **Deterministic Execution**: Typed errors and cleanup guards contribute to deterministic behavior, making MASH reliable even in failure scenarios.

---

## External Commands

A core principle of MASH's safety hardening is to minimize direct shelling out to external commands. Most system utilities that MASH interacts with (e.g., `lsblk`, `parted`, `mount`, `umount`, `losetup`, `xz`) are now wrapped in Rust-native implementations within `mash-hal`. This provides type safety, error handling, and deterministic behavior.

However, a few specialized commands are still invoked externally due to their complexity or unique functionality, but always through carefully designed and safely wrapped interfaces within `mash-hal`:

| Command | Purpose | Notes |
|---------|---------|-------|
| `mkfs.*` | Format various filesystems (e.g., `mkfs.vfat`, `mkfs.ext4`, `mkfs.btrfs`) | Invoked via `mash-hal` with strict parameter validation. |
| `btrfs subvolume` | Create btrfs subvolumes | Invoked via `mash-hal` for btrfs-specific operations. |
| `rsync` | Copy filesystems | Used internally by `mash-hal` for efficient, robust file copying, with parameters carefully controlled. |

These commands are treated as trusted system binaries and are expected to be available on the host system.



---

## Future Considerations

> **Note:** These are architectural observations, not planned features.

-   **Fine-grained HAL Control**: Exposing more granular control over `mash-hal` operations for advanced use cases.
-   **Configurable Partitioning**: While OS-driven, exploring more user-configurable options for partition layouts within the TUI.
-   **Advanced Logging**: Enhancements to the in-TUI log buffer and remote logging capabilities.
-   **Extensibility**: Defining clearer extension points for adding new OSes or custom installation stages.

---

## See Also

- [QUICKSTART.md](QUICKSTART.md) — User guide
- [DOJO.md](DOJO.md) — Development principles
- [DEPLOYMENT.md](DEPLOYMENT.md) — Building and packaging
