# MASH 🍠 v1.4.0

**Minimal, Automated, Self-Hosting installer for Fedora on Raspberry Pi 4B**

MASH is an opinionated installer that automates Fedora KDE installation on Raspberry Pi 4 with UEFI boot support. It is **destructive by design** — it will completely erase and repartition your target disk.

---

## ✨ What MASH Does

- 📥 **Downloads OS Images** — Automatically fetches images for Fedora (KDE, Xfce, LXQt, Minimal, Server), Ubuntu (Server/Desktop), Raspberry Pi OS (Lite/Desktop), and Manjaro (ARM).
- 🗜️ **Decompresses** — Safely extracts `.raw.xz` → `.raw` (or similar formats).
- 🔄 **Loop-mounts** — Mounts the source image for filesystem-level copying (handled via `mash-hal`).
- 💾 **Installs via rsync** — Copies system files preserving permissions and attributes (handled via `mash-hal`).
- 🔧 **Configures UEFI boot** — Ensures `EFI/BOOT/BOOTAA64.EFI` is correctly placed.
- 🌍 **Applies locale settings** — Configures keyboard layout and language.
- ✅ **Supports MBR and GPT** — You choose the partition scheme, guided by OS-specific rules.

---

## 🚀 Two Ways to Run — Always Starts in SAFE MODE

MASH is designed for safety. It always starts in `[SAFE MODE]`, where destructive actions are prevented. You must explicitly arm the installer to proceed with any modifications.

### 1. Interactive TUI Mode (Recommended for one-shot install)

Launch the Dojo UI — it guides you through every step:

```bash
sudo mash
```
You will be prompted to ENABLE DESTRUCTIVE MODE by typing `DESTROY` before any disk modifications.

### 2. CLI Mode (For Scripting — Only use with `---armed` flag)

Fully automated installation with command-line flags. **This mode requires explicit `--armed` flag to perform destructive operations.**

```bash
sudo mash flash \
  --disk /dev/sda \
  --scheme mbr \
  --download-image \
  --download-uefi \
  --auto-unmount \
  --armed \
  --yes-i-know
```

---

## ⚠️ WARNING — DESTRUCTIVE OPERATION

This installer **DESTROYS THE TARGET DISK**.

MASH is designed to protect your running system and boot media. It starts in `[SAFE MODE]`, where no destructive operations can occur. You must explicitly type `DESTROY` to switch to `[ARMED]` mode before any disk modifications.

- All existing data on the target disk will be erased.
- All partitions on the target disk will be deleted.
- There is no undo.

You will be asked to confirm by typing `DESTROY` before enabling destructive actions. **Double-check the device name every time.** The installer will also prevent you from selecting your running system's boot disk by default.

---

## 📦 Partition Layout

MASH now uses OS-specific partitioning rules. The layout will vary depending on the chosen operating system.

**Example: Fedora's Default Layout** (subject to change by Fedora itself)

| Partition | Size | Format | Purpose |
|---|---|---|---|
| EFI | 1 GiB | FAT32 | UEFI boot files |
| BOOT | 2 GiB | ext4 | Kernel and initramfs |
| ROOT | ~1.8 TiB | btrfs | System root (subvols: root, home, var) |
| DATA | Remaining | ext4 | User data and staging (optional) |

**Important OS-Specific Notes:**
- Manjaro ARM images are flashed with their default 2-partition layout; the installer will **not** create a third data partition during installation, but this can be done post-boot.
- Partition sizes are generally configurable via CLI flags (`--efi-size`, `--boot-size`, `--root-end`) for Fedora, but behavior may vary for other OSes based on their requirements.

---

## 🧮 Versioning

MASH uses strict SemVer (`X.Y.Z`) and tags releases as `vX.Y.Z`.

---

## 🔧 Building from Source

### Prerequisites

- Rust toolchain (1.70+)
- System packages: `parted`, `rsync`, `xz`, `mkfs.vfat`, `mkfs.ext4`, `mkfs.btrfs`

### Build Commands

```bash
make build-cli      # Build release binary
make dev-cli        # Build debug binary (faster)
make test           # Run tests
make lint           # Run clippy linter
make format         # Format code
```

The binary is output to `mash-installer/target/release/mash`.

---

## 📚 Documentation

- [Quick Start Guide](docs/QUICKSTART.md) — Get running in minutes
- [Architecture](docs/ARCHITECTURE.md) — Technical design and module structure
- [Deployment](docs/DEPLOYMENT.md) — Packaging and distribution
- [Releasing](docs/RELEASING.md) — Release workflow and tooling
- [Development Principles](docs/DOJO.md) — Code philosophy and rules

---

## 🎯 Design Philosophy

- **User choice is sacred** — MBR vs GPT is always your decision, but guided by OS needs.
- **Destructive actions require explicit confirmation** — Type `DESTROY` to proceed; no silent overwrites.
- **Boot media is protected** — The installer prevents accidental selection of the running system's drive.
- **Rust-native HAL for safety** — All critical system operations are implemented in Rust for robustness, testability, and determinism.
- **Noisy and defensive** — Verbose logging, clear error messages.
- **No surprises** — What you see is what you get.

---

## 📋 System Requirements

**Host machine (where you run MASH):**
- Linux with root access
- 4+ GB RAM recommended
- Network connection (for downloads)

**Target (Raspberry Pi 4B):**
- Raspberry Pi 4 Model B (MASH performs hardware detection and may warn/fail on other models)
- SD card or USB drive (8+ GB minimum, 32+ GB recommended)
- UEFI firmware installed (or it will be downloaded/configured automatically by the installer)

---

## 🐛 Troubleshooting

### "No TTY detected"
MASH TUI requires an interactive terminal. Run directly, not via pipe or script:
```bash
sudo mash           # ✅ Correct
echo | sudo mash    # ❌ Won't work
```

### "Permission denied"
Run with sudo — MASH needs root for disk operations:
```bash
sudo mash
```

### "Disk not found"
Verify your disk is connected and identify it correctly:
```bash
lsblk
```

### "Destructive operation blocked / Not in ARMED mode"
MASH starts in `[SAFE MODE]`. You must explicitly type `DESTROY` at the prompt to enable destructive operations and switch to `[ARMED]` mode. This protects against accidental data loss.

### Checking Logs
All MASH logs are routed to `~/.mash/mash.log`. You can also toggle an in-TUI log buffer by pressing `F12`.

---

## 📄 License

See [LICENSE](LICENSE) for details.

---

> *Anyone can cook. This one just boots cleanly.* 🐀
# codex test
# codex test
# codex test
# codex test
# codex test
# codex test
