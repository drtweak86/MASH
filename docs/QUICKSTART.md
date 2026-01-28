# MASH Installer - Quick Start Guide

## 🚀 Get Started in 3 Steps

### 1. Install MASH Installer

```bash
curl -fsSL https://raw.githubusercontent.com/drtweak86/mash-installer/main/install.sh | sudo bash
```

### 2. Prepare Your Image

Download Fedora KDE for ARM:
- Visit: https://fedoraproject.org/wiki/Architectures/ARM/Raspberry_Pi
- Download the Fedora KDE `.raw` image for Raspberry Pi

### 3. Run the Installer

#### Using GUI (Easiest)

```bash
sudo mash-installer-qt
```

1. Click "Browse" to select your Fedora image
2. Select target disk from dropdown
3. Optionally check "Dry Run" to test first
4. Click "Install" and confirm prompts
5. Wait for completion

#### Using CLI

```bash
# Check system first
mash-installer preflight

# Install to /dev/sda (REPLACE WITH YOUR DISK!)
sudo mash-installer flash \
  --image ~/Downloads/Fedora-KDE-40.raw \
  --disk /dev/sda \
  --uefi-dir /path/to/uefi \
  --auto-unmount \
  --yes-i-know
```

## ⚠️ Important Notes

### Before You Start

1. **Backup Data**: This will ERASE the target disk completely!
2. **UEFI Firmware**: Raspberry Pi 4 must have UEFI firmware (not U-Boot)
   - Download from: https://github.com/pftf/RPi4/releases
   - Flash to SD card first, then boot to install UEFI
3. **Disk Size**: Minimum 3.7TB for full partition layout
4. **Double-Check Disk**: Use `lsblk` to verify correct disk before installing

### Disk Selection Tips

```bash
# List all disks
lsblk

# Identify your SD card/USB drive
# Look for removable devices or matching size
# Example output:
# sda           8:0    1  119.2G  0 disk   ← SD card
# └─sda1        8:1    1  119.2G  0 part
# nvme0n1     259:0    0  476.9G  0 disk   ← Your laptop SSD (DON'T USE!)
```

### Common Disk Names

- **SD Card**: `/dev/mmcblk0`
- **USB Drive**: `/dev/sda`, `/dev/sdb`
- **NVMe**: `/dev/nvme0n1` (usually your system drive - avoid!)

## 📋 What Happens During Installation

1. ✅ System checks (preflight)
2. 🧹 Unmount existing partitions
3. 🔧 Create 4 partitions (MBR):
   - EFI: 512 MB (FAT32)
   - BOOT: 1 GB (ext4)
   - ROOT: 1.8 TB (ext4)
   - DATA: Remaining space (ext4)
4. 💾 Format all partitions
5. 📦 Copy Fedora system from image
6. ⚙️ Configure UEFI boot (dracut, GRUB)
7. 🎉 Done!

## 🔍 Verify Installation

After installation completes:

```bash
# Check partitions were created
sudo parted /dev/sda print

# Verify filesystems
sudo blkid | grep sda
```

You should see 4 partitions with correct labels:
- sda1: EFI
- sda2: BOOT
- sda3: ROOT
- sda4: DATA

## 🎮 First Boot

1. Safely eject the SD card/USB drive
2. Insert into Raspberry Pi 4
3. Connect HDMI, keyboard, mouse
4. Power on
5. System should boot to Fedora KDE login screen
6. Default user: fedora (no password on first boot)

## 🐛 Troubleshooting

### "Permission denied"
```bash
# Make sure you're using sudo
sudo mash-installer-qt
```

### "Disk not found"
```bash
# Refresh and check disk name
lsblk
# Make sure disk is connected
# Try with full path: /dev/sda instead of just sda
```

### "Image file not found"
```bash
# Verify file exists and path is correct
ls -lh ~/Downloads/*.raw
# Use absolute path in installer
```

### "UEFI boot failed"
- Ensure UEFI firmware is installed on RPi4
- Check UEFI version is compatible
- Try re-flashing UEFI firmware

### Installation hangs
- Check system logs: `journalctl -f`
- Verify disk is not failing: `sudo smartctl -a /dev/sda`
- Ensure sufficient power supply (RPi4 needs 5V/3A)

## 📚 Learn More

- **Full Documentation**: [README.md](../README.md)
- **Architecture**: [docs/ARCHITECTURE.md](ARCHITECTURE.md)
- **Report Issues**: https://github.com/drtweak86/mash-installer/issues

## 💡 Pro Tips

### Test First with Dry Run

```bash
# This won't make any changes - safe to test
sudo mash-installer flash \
  --image ~/Downloads/Fedora.raw \
  --disk /dev/sda \
  --uefi-dir /path/to/uefi \
  --dry-run
```

### Save Time on Repeat Installs

```bash
# Store your common command in a script
cat > ~/install-fedora.sh <<'EOF'
#!/bin/bash
sudo mash-installer flash \
  --image ~/Downloads/Fedora-KDE-40.raw \
  --disk /dev/sda \
  --uefi-dir ~/uefi-firmware \
  --auto-unmount \
  --yes-i-know
EOF

chmod +x ~/install-fedora.sh
# Then just run: ~/install-fedora.sh
```

### Monitor Progress

```bash
# In another terminal, watch the process
watch -n 2 'lsblk; echo; df -h'
```

## 🎯 Next Steps After Installation

1. **Update System**
   ```bash
   sudo dnf update -y
   ```

2. **Configure WiFi** (if needed)
   ```bash
   nmtui
   ```

3. **Install Additional Software**
   ```bash
   sudo dnf install vim htop neofetch
   ```

4. **Enable SSH** (for remote access)
   ```bash
   sudo systemctl enable --now sshd
   ```

5. **Set up user account**
   ```bash
   sudo useradd -m -G wheel yourusername
   sudo passwd yourusername
   ```

---

**Ready to install? Let's go! 🚀**
