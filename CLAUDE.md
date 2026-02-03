# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

MASH (Minimal, Automated, Self-Hosting) is a Rust-based installer that automates OS installation on Raspberry Pi 4B with UEFI boot support. It now supports Fedora KDE, Ubuntu, Raspberry Pi OS, and Manjaro.

The installer is **destructive by design**, but strictly enforces a **SAFE MODE** initially. Destructive actions require explicit user confirmation via an **ARMING SEQUENCE** (typing `DESTROY`). This is a safety-critical feature.

**Current version:** v1.4.0

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

MASH operates with a strong emphasis on safety and an enforced workflow.

1.  **TUI Mode** (default, no subcommand): Interactive terminal wizard.
    -   Always starts in **`[SAFE MODE]`** (read-only).
    -   Requires an **Arming Sequence** (typing `DESTROY`) to switch to **`[ARMED]`** mode for destructive operations.
    -   Enforces a sequential workflow: Welcome → Distro → Flavour → Disk → Partition → Review.
    -   Protects the installer's boot media from accidental selection.

2.  **CLI Mode** (subcommands: `preflight`, `flash`): For scripting and automation.
    -   Requires explicit `--armed` flag to perform destructive operations.

### Module Structure

MASH is organized as a Cargo workspace with dedicated crates for different functionalities:

```
.
├── mash-installer/             # Main executable (thin orchestration)
├── mash-core/                  # Shared types, errors, config, download logic
└── crates/
    ├── mash-hal/               # Hardware Abstraction Layer (system ops)
    ├── mash-tui/               # Terminal User Interface components (UI, widgets, input)
    └── mash-workflow/          # Stage engine, state management, install pipelines
```
`mash-tui` is the primary crate for all UI-related development.

### TUI Architecture (crates/mash-tui)

The TUI (Terminal User Interface) is built using `ratatui` within the `crates/mash-tui` crate. It implements a purposeful layout grid to provide clear user guidance and real-time system information.

**UI Layout Grid:**
-   **Left Sidebar**: Displays the step progress (Welcome → Distro → Flavour → Disk → Partition → Review).
-   **Center**: The active screen for user interaction (e.g., distro selection, disk partitioning).
-   **Right Panel**: Provides essential system information (RAM, CPU, detected disks, current Safety State).
-   **Bottom Bar**: Shows key legends (`Space`, `Enter`, `Esc`, `q`) for navigation and actions.

**Enforced Workflow:**
The TUI strictly guides the user through the following sequence, ensuring no steps are missed and critical safety checks are performed:
1.  **Welcome**: Initial greeting and overview.
2.  **Distro Selection**: Choose the operating system. Only distros with valid download URLs and checksums are presented; no placeholders.
3.  **Flavour / Edition Selection**: Context-aware variants appear after distro selection (e.g., Fedora → Server / Workstation / KDE; Raspberry Pi OS → Lite / Desktop; Ubuntu → Server / Desktop).
4.  **Disk Selection**: Choose the target disk. The installer's source drive (boot media) is automatically excluded.
5.  **Partition Wizard**: Select partitioning strategy (Whole Disk or Manual Layout). EFI options are shown only if required by the board/distro metadata.
6.  **Review**: Final confirmation of all selections before proceeding.
7.  **Installation Progress**: Displays real-time progress updates.

This architecture ensures a clear, safe, and guided installation experience, with all destructive actions gated by the `DESTROY` arming sequence.

### Key Types (Relevant to TUI)

-   **`InstallConfig`** (from `mash-core`): Accumulates user configuration choices (OS, variant, target disk, partitioning scheme, locale, etc.).
-   **`InstallStep`** (from `mash-workflow`): Represents the current stage of the enforced workflow (e.g., `DistroSelection`, `PartitionWizard`).
-   **`ProgressUpdate`** / **`Phase`** (from `mash-workflow`): Communication mechanism for progress from the background installation thread to the TUI.
-   **`MASHError`** (from `mash-core`): Standardized error types, including `SafetyLock`, `DiskBusy`, etc.

### Installation Pipeline

The core installation logic is now orchestrated by `crates/mash-workflow` and executed via `crates/mash-hal` for system operations. This ensures a robust, testable, and deterministic process.

The TUI (driven by `crates/mash-tui`) communicates with `mash-workflow` to initiate and monitor the stages of the installation, which include:

1.  **Preflight Checks**: Verifying system readiness.
2.  **OS/Image Download**: Fetching necessary OS images.
3.  **Source Image Preparation**: Setting up the source data.
4.  **Target Disk Partitioning**: Creating the disk layout.
5.  **Target Filesystem Formatting**: Preparing filesystems.
6.  **Filesystem Copy**: Transferring OS files.
7.  **UEFI Boot Configuration**: Setting up bootloaders.
8.  **System Configuration & Locale**: Applying post-install settings.
9.  **Cleanup**: Releasing resources.

### Progress Communication

Downloads and flashing run in separate threads, sending `ProgressUpdate` messages via `mpsc::Sender` to keep the TUI responsive.

## Core Principles (from docs/DOJO.md)

-   **Destructive actions must be explicit** — Always require an explicit `DESTROY` arming sequence.
-   **GPT/MBR choice is guided by OS rules** — The installer will guide the user and may enforce OS-specific rules.
-   **Noisy, clear, and defensive** — Verbose logging, clear error messages.
-   **No surprises** — What you see is what you get.
-   **Overwrites must create a `bak/` mirror** — Preserve previous state (where applicable).

## External Dependencies

Most interactions with system utilities (e.g., `lsblk`, `parted`, `mkfs.*`, `mount`, `rsync`) are now encapsulated within the `crates/mash-hal` Hardware Abstraction Layer. This means that Claude (as the UI developer) can rely on `mash-hal` to perform these operations safely and reliably without needing to understand the underlying shell calls. This separation of concerns simplifies UI development and ensures robustness.

## CLI Flags Reference

```
mash flash [OPTIONS] --disk <DISK>

Options:
  --armed                  REQUIRED: Confirm destructive operation; enables ARMRED mode.
  --distro <NAME>          Operating System to install (e.g., fedora, ubuntu, rpios, manjaro)
  --flavour <NAME>         OS Edition/Flavour (e.g., kde, desktop, server, lite)
  --image <PATH>           Path to local OS image file
  --disk <DEVICE>          Target disk (e.g., /dev/sda). Will be protected if it is the source drive.
  --scheme <mbr|gpt>       Partition scheme (default: mbr). Will be guided by OS rules.
  --efi-size <SIZE>        EFI partition size (default: 1024MiB)
  --boot-size <SIZE>       BOOT partition size (default: 2048MiB)
  --root-end <SIZE>        ROOT partition end (default: 1800GiB)
  --locale <LANG:KEYMAP>   Locale setting (e.g., en_GB.UTF-8:uk)
  --early-ssh              Enable SSH before graphical login
  --dry-run                Simulate without changes
```

## Testing Notes



-   **Always use `--dry-run` flag** when testing installation logic.

-   **UI Testing**: When testing UI components or flows, `mash-hal` provides `MockDiskOps` to simulate disk operations without touching real hardware, ensuring CI safety.

-   Unit tests exist in `locale.rs` for locale patching.

-   Full installation testing requires physical media (too dangerous to automate).

-   Run `make lint` before committing — CI enforces `-D warnings`.

## Git Workflow

- `main` branch is the source of truth
- Backup files go in `bak/` directory
- Legacy scripts preserved in `archive/legacy_scripts/HISTORY/`
