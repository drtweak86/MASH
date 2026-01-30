# MASH Quick Start Guide 🚀

Get Fedora running on your Raspberry Pi 4 in minutes.

---

## ⚠️ Before You Begin

**This installer will ERASE your target disk completely.**

- Back up any important data first
- Double-check your disk device name
- There is no undo

---

## 📋 Prerequisites

### On Your Host Machine (Linux)

You need these packages installed:

```bash
# Debian/Ubuntu
sudo apt install parted rsync xz-utils dosfstools e2fsprogs btrfs-progs

# Fedora
sudo dnf install parted rsync xz dosfstools e2fsprogs btrfs-progs

# Arch
sudo pacman -S parted rsync xz dosfstools e2fsprogs btrfs-progs
```

### Target Hardware

- Raspberry Pi 4 Model B
- SD card or USB drive (minimum 8 GB, recommended 32+ GB)
- HDMI cable, keyboard, and mouse for first boot

---

## 🎯 Step 1: Identify Your Target Disk

Connect your SD card or USB drive and identify it:

```bash
lsblk
```

Example output:
```
NAME        SIZE  TYPE  MOUNTPOINT
sda         32G   disk            ← Your SD card (USE THIS)
├─sda1       1G   part
└─sda2      31G   part
nvme0n1    500G   disk            ← System drive (DON'T USE!)
├─nvme0n1p1  1G   part  /boot/efi
└─nvme0n1p2 499G  part  /
```

**Common disk names:**
- SD card in USB reader: `/dev/sda`, `/dev/sdb`
- SD card in built-in slot: `/dev/mmcblk0`
- USB drive: `/dev/sda`, `/dev/sdb`
- NVMe (usually your system — avoid!): `/dev/nvme0n1`

---

## 🎯 Step 2: Run MASH

### Option A: Interactive TUI (Recommended)

Launch the wizard and follow the prompts:

```bash
sudo mash
```

The TUI guides you through:
1. Selecting your target disk
2. Choosing image source (download or local)
3. Selecting partition scheme (MBR recommended)
4. Confirming you have backed up your data (required)
5. Creating your first-boot user (no autologin)
6. Final confirmation before flashing

### Option B: CLI Mode (For Automation)

Run with all options on the command line:

```bash
sudo mash flash \
  --disk /dev/sda \
  --scheme mbr \
  --download-image \
  --download-uefi \
  --auto-unmount \
  --yes-i-know
```

This will:
1. Download Fedora 43 KDE image
2. Download latest UEFI firmware
3. Partition and format `/dev/sda`
4. Install Fedora with UEFI boot

---

## 🧪 Step 3: Test First (Optional but Recommended)

Run a dry-run to see what would happen without making changes:

```bash
sudo mash flash \
  --disk /dev/sda \
  --scheme mbr \
  --download-image \
  --download-uefi \
  --auto-unmount \
  --yes-i-know \
  --dry-run
```

---

## ✅ Step 4: Verify Installation

After flashing completes, verify the partitions:

```bash
sudo parted /dev/sda print
```

You should see 4 partitions:
```
Number  Start   End     Size    Type     File system  Flags
 1      1049kB  1075MB  1074MB  primary  fat32        boot, esp
 2      1075MB  3222MB  2147MB  primary  ext4
 3      3222MB  1933GB  1930GB  primary  btrfs
 4      1933GB  2000GB  67.1GB  primary  ext4
```

---

## 🔌 Step 5: First Boot

1. **Safely eject** the SD card/USB drive from your host
2. **Insert** into your Raspberry Pi 4
3. **Connect** HDMI, keyboard, and mouse
4. **Power on** the Pi

The system should boot to:
- UEFI firmware screen briefly
- GRUB bootloader
- Fedora KDE desktop

---

## 🔧 CLI Options Reference

| Flag | Description | Default |
|------|-------------|---------|
| `--disk <DEVICE>` | Target disk device | Required |
| `--scheme <mbr\|gpt>` | Partition table type | `mbr` |
| `--image <PATH>` | Local .raw image file | — |
| `--download-image` | Auto-download Fedora | — |
| `--download-uefi` | Auto-download UEFI firmware | — |
| `--image-version <VER>` | Fedora version | `43` |
| `--image-edition <ED>` | Fedora edition | `KDE` |
| `--uefi-dir <PATH>` | Local UEFI files directory | — |
| `--auto-unmount` | Unmount disk before flashing | — |
| `--yes-i-know` | Confirm destructive operation | Required |
| `--locale <LANG:KEYMAP>` | Set locale (e.g., `en_GB.UTF-8:uk`) | — |
| `--early-ssh` | Enable SSH on first boot | — |
| `--dry-run` | Simulate without changes | — |

---

## 🐛 Troubleshooting

### "No TTY detected"

MASH TUI requires an interactive terminal:
```bash
# Run directly in terminal
sudo mash

# Won't work via pipe
cat | sudo mash  # ❌
```

### "Permission denied"

Always run with sudo:
```bash
sudo mash
```

### "Disk is busy" / "Target is mounted"

Unmount all partitions first:
```bash
sudo umount /dev/sda*
```

Or use `--auto-unmount` flag.

### "Image file not found"

Verify your image path:
```bash
ls -lh ~/Downloads/*.raw
```

Use the full absolute path, or use `--download-image` to auto-download.

### UEFI boot fails

- Ensure your Pi has UEFI firmware (not default U-Boot)
- Try re-flashing with `--download-uefi` to get latest firmware
- Check that EFI partition has `BOOTAA64.EFI` in correct location

### Boot hangs at black screen

- Ensure adequate power supply (5V/3A)
- Try MBR scheme if GPT doesn't work: `--scheme mbr`
- Check HDMI cable and monitor compatibility

---

## 🎉 Post-Installation

After first boot, you may want to:

```bash
# Update system
sudo dnf update -y

# Set up WiFi
nmtui

# Enable SSH for remote access
sudo systemctl enable --now sshd

# Create your user account
sudo useradd -m -G wheel yourusername
sudo passwd yourusername
```

---

## 📚 More Information

- [Main README](../README.md) — Project overview
- [Architecture](ARCHITECTURE.md) — Technical details
- [Deployment](DEPLOYMENT.md) — Building and packaging
- [Development Principles](DOJO.md) — Code philosophy

---

**Ready to install?** Run `sudo mash` and follow the wizard! 🍠
