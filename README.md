# MASH 🍠 v1.2.11

**Minimal, Automated, Self-Hosting installer for Fedora on Raspberry Pi 4B**

MASH is an opinionated installer that automates Fedora KDE installation on Raspberry Pi 4 with UEFI boot support. It is **destructive by design** — it will completely erase and repartition your target disk.

---

## ✨ What MASH Does

- 📥 **Downloads Fedora** — Automatically fetches Fedora 42/43 aarch64 images (KDE, Xfce, LXQt, Minimal, Server)
- 📥 **Downloads UEFI firmware** — Fetches the latest RPi4 UEFI firmware from GitHub
- 🗜️ **Decompresses** — Safely extracts `.raw.xz` → `.raw`
- 🔄 **Loop-mounts** — Mounts the source image for filesystem-level copying
- 💾 **Installs via rsync** — Copies system files preserving permissions and attributes
- 🔧 **Configures UEFI boot** — Ensures `EFI/BOOT/BOOTAA64.EFI` is correctly placed
- 🌍 **Applies locale settings** — Configures keyboard layout and language
- ✅ **Supports MBR and GPT** — You choose the partition scheme

---

## 🚀 Two Ways to Run

### 1. Interactive TUI Mode (Recommended)

Launch the terminal wizard — it guides you through every step:

```bash
sudo mash
```

### 2. CLI Mode (For Scripting)

Fully automated installation with command-line flags:

```bash
sudo mash flash \
  --disk /dev/sda \
  --scheme mbr \
  --download-image \
  --download-uefi \
  --auto-unmount \
  --yes-i-know
```

---

## ⚠️ WARNING — DESTRUCTIVE OPERATION

This installer **DESTROYS THE TARGET DISK**.

- All existing data will be erased
- All partitions will be deleted
- There is no undo

You will be asked to confirm before any destructive action. **Double-check the device name every time.**

---

## 📦 Partition Layout

MASH creates a 4-partition layout:

| Partition | Size | Format | Purpose |
|-----------|------|--------|---------|
| EFI | 1 GiB | FAT32 | UEFI boot files |
| BOOT | 2 GiB | ext4 | Kernel and initramfs |
| ROOT | ~1.8 TiB | btrfs | System root (subvols: root, home, var) |
| DATA | Remaining | ext4 | User data and staging |

Partition sizes are configurable via CLI flags (`--efi-size`, `--boot-size`, `--root-end`).

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
- [Development Principles](docs/DOJO.md) — Code philosophy and rules

---

## 🎯 Design Philosophy

- **User choice is sacred** — MBR vs GPT is always your decision
- **Destructive actions require explicit confirmation** — No silent overwrites
- **Noisy and defensive** — Verbose logging, clear error messages
- **No surprises** — What you see is what you get

---

## 📋 System Requirements

**Host machine (where you run MASH):**
- Linux with root access
- 4+ GB RAM recommended
- Network connection (for downloads)

**Target (Raspberry Pi 4B):**
- Raspberry Pi 4 Model B
- SD card or USB drive (8+ GB minimum, 32+ GB recommended)
- UEFI firmware installed (or use `--download-uefi`)

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

---

## 📄 License

See [LICENSE](LICENSE) for details.

---

> *Anyone can cook. This one just boots cleanly.* 🐀
