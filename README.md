# 🦀 MASH Installer v1.0

**Full-Loop Fedora KDE Installer for Raspberry Pi 4B with UEFI Boot + Dojo Post-Install System**

## 🎯 What Is This?

MASH is a complete solution for installing Fedora KDE on Raspberry Pi 4B that combines:

1. **Rust CLI/Qt GUI Installer** - Flash Fedora images with proper UEFI boot
2. **Loop Mount System** - No extraction needed, works directly with .raw images  
3. **Btrfs with Subvolumes** - Modern filesystem with snapshots via Snapper
4. **MBR 4-Partition Layout** - Optimized for 4TB drives
5. **Dojo Post-Install System** - Automated fixes, packages, and configurations
6. **CI/CD Pipeline** - Automated builds for ARM64 and x86_64

## 🏗️ Architecture

### Partition Layout (MBR)

```
/dev/sda or /dev/mmcblk0
├─ p1: EFI     1 GB    FAT32   boot flag   /boot/efi
├─ p2: BOOT    2 GB    ext4                /boot
├─ p3: ROOT    1.8 TB  btrfs               / (subvol=root), /home (subvol=home)
└─ p4: DATA    ~2 TB   ext4    LABEL=DATA  /data
```

### Installation Flow

```
User Input → Preflight Checks → Wipe & Partition (MBR) →
Format (btrfs + subvols) → Loop Mount Image → rsync System →
UEFI Config (dracut + GRUB) → Stage Dojo to /data → 
Offline Boot Units → First Boot → Dojo Appears
```

### Dojo System

The Dojo is a post-install automation system that runs after first boot:

**Location**: `/data/mash-staging/` (staged during install)

**Features**:
- 🖥️ TUI menu system with ASCII art
- 📦 Package installation (core, dev, desktop)
- 🔧 System fixes (Argon ONE, screensaver, firewall)
- 🌐 Brave browser installation  
- 🎨 ZSH + Starship prompt
- 📸 Snapper snapshot configuration
- 🔒 Fail2ban lite
- 🌍 UK locale defaults
- 🚀 Early SSH (available immediately on boot)

**Manual Launch**: `/usr/local/bin/mash-dojo-launch`

**Auto-Launch**: Configured via `/etc/xdg/autostart/mash-dojo.desktop`

## ✨ Features

### Core Installer
- ✅ Loop mount support (no image extraction)
- ✅ MBR partitioning (4 partitions)
- ✅ Btrfs with subvolumes (root, home)
- ✅ UEFI boot configuration (dracut + GRUB ARM64-EFI)
- ✅ UUID-based fstab
- ✅ Offline locale patching (en_GB.UTF-8, gb keymap)
- ✅ Safety features (dry-run, confirmations, disk verification)

### Qt GUI
- ✅ Modern interface with live logging
- ✅ Disk auto-discovery
- ✅ Progress tracking
- ✅ Double confirmation dialogs
- ✅ pkexec privilege elevation

### Dojo Post-Install
- ✅ Modular helper script system
- ✅ TUI menu with categories
- ✅ Package groups (core, dev, desktop)
- ✅ Hardware fixes (Argon ONE fan control)
- ✅ Browser setup (Brave)
- ✅ Shell customization (ZSH + Starship)
- ✅ Snapshot configuration (Snapper)
- ✅ Security (fail2ban lite, firewall)
- ✅ Early boot SSH

### CI/CD
- ✅ GitHub Actions automation
- ✅ Cross-compilation (ARM64 + x86_64)
- ✅ Automatic releases on version tags
- ✅ Binary artifacts with checksums

## 🚀 Quick Start

### One-Command Install

```bash
curl -fsSL https://raw.githubusercontent.com/drtweak86/mash-installer/main/install.sh | sudo bash
```

### Usage

#### GUI (Recommended)

```bash
sudo mash-installer-qt
```

1. Select Fedora KDE .raw image
2. Choose target disk
3. Verify UEFI directory
4. Click "Install"
5. Confirm warnings
6. Wait for completion

#### CLI

```bash
# Preflight check
mash-installer preflight

# Dry run (safe test)
sudo mash-installer flash \
  --image ~/Fedora-KDE-40.raw \
  --disk /dev/sda \
  --uefi-dir ~/rpi4-uefi \
  --dry-run

# Real installation
sudo mash-installer flash \
  --image ~/Fedora-KDE-40.raw \
  --disk /dev/sda \
  --uefi-dir ~/rpi4-uefi \
  --auto-unmount \
  --yes-i-know
```

## 📦 What Gets Installed

### System Files (During Flash)

```
/boot/efi/               # UEFI firmware (RPI_EFI.fd, start4.elf, etc.)
/boot/                   # Kernels, initramfs
/                        # Fedora KDE root (btrfs subvol=root)
/home/                   # User home directories (btrfs subvol=home)
/data/mash-staging/      # Dojo bundle + helpers
/data/mash-logs/         # First-boot logs
```

### Dojo System Files (Offline Install)

```
/usr/local/bin/mash-dojo-launch              # Launcher script
/usr/local/lib/mash/dojo/                    # Dojo modules
  ├── dojo.sh                                # Main menu
  ├── menu.sh                                # Menu system
  ├── bootstrap.sh                           # Bootstrap runner
  ├── argon_one.sh                           # Fan control
  ├── browser.sh                             # Brave installer
  ├── snapper.sh                             # Snapshot config
  ├── firewall.sh                            # Firewall setup
  └── ...
/usr/local/lib/mash/system/                  # System scripts
  ├── early-ssh.sh                           # SSH early boot
  └── internet-wait.sh                       # Network wait
/etc/xdg/autostart/mash-dojo.desktop         # Autostart config
/etc/systemd/system/mash-early-ssh.service   # Early SSH unit
/etc/systemd/system/mash-internet-wait.service
```

### Helper Scripts (Staged to /data)

```
/data/mash-staging/helpers/
├── 00_write_config_txt.sh      # RPi config.txt
├── 01_stage_bootstrap.sh       # Bootstrap staging
├── 02_early_ssh.sh             # SSH setup
├── 02_internet_wait.sh         # Network wait
├── 03_fail2ban_lite.sh         # Simple fail2ban
├── 10_locale_uk.sh             # UK locale
├── 11_snapper_init.sh          # Snapper init
├── 12_firewall_sane.sh         # Firewall config
├── 13_packages_core.sh         # Core packages
├── 14_packages_dev.sh          # Dev tools
├── 15_packages_desktop.sh      # Desktop apps
├── 16_mount_data.sh            # DATA mount
├── 17_brave_browser.sh         # Brave install
├── 17_brave_default.sh         # Set Brave default
├── 20_argon_one.sh             # Argon ONE driver
├── 21_zsh_starship.sh          # ZSH setup
└── 22_kde_screensaver_nuke.sh  # Disable screensaver
```

## 🎮 Using Dojo After First Boot

### Automatic Launch

After first login to Fedora KDE, Dojo will automatically appear as a desktop notification/popup.

### Manual Launch

```bash
# Launch Dojo TUI
/usr/local/bin/mash-dojo-launch

# Or install from staging
sudo /data/mash-staging/install_dojo.sh /data/mash-staging
```

### Dojo Menu Options

```
╔══════════════════════════════════════╗
║     🥋 MASH Dojo - System Setup     ║
╚══════════════════════════════════════╝

1. 📦 Install Core Packages
2. 🔧 Install Development Tools  
3. 🎨 Install Desktop Applications
4. 🌐 Install Brave Browser
5. 🎯 Configure Argon ONE Fan
6. 💻 Setup ZSH + Starship
7. 📸 Initialize Snapper
8. 🔒 Configure Firewall
9. 🚫 Disable KDE Screensaver
10. 🌍 Set UK Locale
11. 💾 Mount DATA Partition
12. 🔄 Run All Bootstrap
Q. Quit
```

## 📚 Dojo Helper Details

### Package Groups

**Core Packages** (`13_packages_core.sh`):
- vim, htop, tmux, git, curl, wget
- rsync, tree, lsof, strace
- Build essentials

**Dev Tools** (`14_packages_dev.sh`):
- GCC, clang, rust, go, python3-devel
- cmake, make, ninja-build
- gdb, valgrind

**Desktop Apps** (`15_packages_desktop.sh`):
- LibreOffice, GIMP, Inkscape
- VLC, Audacity
- Thunderbird, Transmission

### System Fixes

**Argon ONE** (`20_argon_one.sh`):
- Installs fan control driver
- Configures temperature thresholds
- Enables power button support

**Firewall** (`12_firewall_sane.sh`):
- Enables firewalld
- Opens SSH (22)
- Configures sensible defaults

**Screensaver Nuke** (`22_kde_screensaver_nuke.sh`):
- Disables KDE lockscreen
- Removes screensaver timeout
- Prevents screen blanking

### Early Boot Features

**Early SSH** (`mash-early-ssh.service`):
- Starts SSH as soon as network is up
- No need to wait for full boot
- Useful for headless setups

**Internet Wait** (`mash-internet-wait.service`):
- Waits for internet connectivity
- Blocks dependent services
- Ensures network-dependent tasks succeed

## 🛠️ Development

### Build Locally

```bash
# Install dependencies (Fedora)
sudo dnf install -y rust cargo cmake qt6-qtbase-devel

# Build
make

# Or individually
make build-cli    # Rust CLI
make build-qt     # Qt GUI

# Install
sudo make install
```

### Project Structure

```
mash-merged/
├── mash-installer/          # Rust CLI
│   ├── src/
│   │   ├── main.rs
│   │   ├── cli.rs
│   │   ├── flash.rs        # Core installer (loop mount, btrfs, UEFI)
│   │   ├── preflight.rs
│   │   ├── errors.rs
│   │   └── logging.rs
│   └── Cargo.toml
├── qt-gui/                  # Qt GUI wrapper
│   ├── src/
│   │   ├── main.cpp
│   │   ├── mainwindow.cpp
│   │   ├── mainwindow.h
│   │   └── mainwindow.ui
│   └── CMakeLists.txt
├── dojo_bundle/             # Dojo system
│   ├── usr_local_bin/
│   ├── usr_local_lib_mash/
│   ├── systemd/
│   ├── autostart/
│   ├── assets/
│   └── install_dojo.sh
├── helpers/                 # Helper scripts
│   ├── 00_write_config_txt.sh
│   ├── 02_early_ssh.sh
│   ├── 13_packages_core.sh
│   └── ...
├── .github/workflows/       # CI/CD
│   └── build.yml
├── Makefile
├── install.sh
└── README.md
```

### Version Bumping

```bash
# Bump version
./scripts/bump-version.sh patch

# Push to trigger release
git push origin main --tags
```

## 🔒 Safety Features

1. **`--yes-i-know` flag** - Required for destructive operations
2. **lsblk verification** - Shows disk layout before proceeding
3. **Double confirmation** - GUI requires two explicit confirmations
4. **Dry-run mode** - Test without making changes
5. **Disk validation** - Checks block device exists
6. **UEFI verification** - Validates firmware files present
7. **Mount point cleanup** - Unmounts everything properly

## 📋 Requirements

### Hardware
- Raspberry Pi 4B (4GB or 8GB RAM recommended)
- 4TB+ storage (SD card or USB SSD/HDD)
- UEFI firmware installed (not U-Boot)

### Software (Host)
- Linux system (for running installer)
- Rust 1.70+ (for building)
- Qt 6.x (for GUI)
- System tools: parted, mkfs.btrfs, mkfs.ext4, mkfs.vfat, rsync, losetup

### UEFI Firmware
Download from: https://github.com/pftf/RPi4/releases

Required files:
- RPI_EFI.fd
- start4.elf
- fixup4.dat
- config.txt
- bcm2711-rpi-4-b.dtb

## 🐛 Troubleshooting

### "Loop device busy"
```bash
sudo losetup -D  # Detach all loop devices
```

### "Partition already mounted"
```bash
sudo umount -R /tmp/mash_*
```

### "Btrfs mount failed"
```bash
# Mount top-level, then subvol
sudo mount /dev/sda3 /mnt
sudo mount -o subvol=root /dev/sda3 /mnt/root
```

### "Dojo not appearing"
```bash
# Manual install
sudo /data/mash-staging/install_dojo.sh /data/mash-staging

# Manual launch
/usr/local/bin/mash-dojo-launch
```

### "GRUB not booting"
- Verify UEFI firmware is installed
- Check `/boot/efi` contents
- Try regenerating: `grub2-mkconfig -o /boot/grub2/grub.cfg`

## 📖 Documentation

- **README.md** (this file) - Overview and quick start
- **docs/ARCHITECTURE.md** - Technical details
- **docs/QUICKSTART.md** - Step-by-step guide
- **docs/DEPLOYMENT.md** - GitHub setup
- **docs/DOJO.md** - Dojo system details

## 🙏 Credits

- **Fedora Project** - Amazing ARM support
- **Raspberry Pi Foundation** - Hardware
- **PFTF UEFI** - UEFI firmware for RPi
- **Rust Community** - Excellent tooling
- **Qt Project** - Cross-platform framework

## 📝 License

MIT License - see LICENSE file

## 🔗 Links

- Repository: https://github.com/drtweak86/mash-installer
- Issues: https://github.com/drtweak86/mash-installer/issues
- UEFI Firmware: https://github.com/pftf/RPi4
- Fedora ARM: https://fedoraproject.org/wiki/Architectures/ARM

---

Made with ❤️, 🦀 Rust, and 🥋 Dojo spirit

**Ready to forge your MASH!**
