# 🥋 MASH Dojo System

The Dojo is a post-install automation and configuration system for MASH-installed Fedora KDE systems on Raspberry Pi 4.

## Overview

**Dojo** (道場, "place of the way") provides a TUI menu system for:
- Installing software packages
- Applying system fixes
- Configuring hardware (Argon ONE)
- Setting up development tools
- Customizing the shell environment

## Installation

### Automatic (During Flash)

The Dojo is automatically staged to `/data/mash-staging/` during the flash process and installed offline into the target system.

### Manual Installation

If you need to install or reinstall Dojo:

```bash
sudo /data/mash-staging/install_dojo.sh /data/mash-staging
```

## Usage

### Auto-Launch

Dojo is configured to appear automatically on first login via:
`/etc/xdg/autostart/mash-dojo.desktop`

### Manual Launch

```bash
# Launch Dojo TUI
/usr/local/bin/mash-dojo-launch

# Or directly
/usr/local/lib/mash/dojo/dojo.sh
```

## Architecture

### File Locations

```
/usr/local/bin/
  └── mash-dojo-launch                    # Launcher script

/usr/local/lib/mash/dojo/                 # Dojo modules
  ├── dojo.sh                             # Main menu
  ├── menu.sh                             # Menu rendering
  ├── bootstrap.sh                        # Bootstrap runner
  ├── argon_one.sh                        # Argon ONE fan control
  ├── browser.sh                          # Browser installation
  ├── snapper.sh                          # Snapshot management
  ├── firewall.sh                         # Firewall configuration
  ├── graphics.sh                         # Graphics fixes
  ├── audio.sh                            # Audio configuration
  ├── mount_data.sh                       # DATA partition mounting
  ├── rclone.sh                           # Cloud storage
  └── borg.sh                             # Backup configuration

/usr/local/lib/mash/system/               # System scripts
  ├── early-ssh.sh                        # Early SSH enablement
  └── internet-wait.sh                    # Network wait helper

/usr/local/lib/mash/dojo/assets/          # Assets
  └── starship.toml                       # Starship prompt config

/etc/xdg/autostart/
  └── mash-dojo.desktop                   # Auto-start config

/etc/systemd/system/
  ├── mash-early-ssh.service              # Early SSH service
  └── mash-internet-wait.service          # Network wait service

/data/mash-staging/                       # Staged installation files
  ├── install_dojo.sh                     # Installer script
  ├── helpers/                            # Helper scripts (00-22)
  ├── usr_local_bin/
  ├── usr_local_lib_mash/
  ├── systemd/
  ├── autostart/
  └── assets/
```

## Menu System

### Main Menu

```
╔══════════════════════════════════════════════════════════╗
║                                                          ║
║              🥋 MASH Dojo - System Setup 🥋             ║
║                  Fedora KDE on RPi4                      ║
║                                                          ║
╠══════════════════════════════════════════════════════════╣
║                                                          ║
║  📦 PACKAGES                                             ║
║    1. Install Core Packages                             ║
║    2. Install Development Tools                         ║
║    3. Install Desktop Applications                      ║
║                                                          ║
║  🌐 BROWSERS                                             ║
║    4. Install Brave Browser                             ║
║    5. Set Brave as Default                              ║
║                                                          ║
║  🔧 HARDWARE                                             ║
║    6. Configure Argon ONE Fan                           ║
║                                                          ║
║  💻 SHELL                                                ║
║    7. Setup ZSH + Starship                              ║
║                                                          ║
║  📸 SNAPSHOTS                                            ║
║    8. Initialize Snapper                                ║
║                                                          ║
║  🔒 SECURITY                                             ║
║    9. Configure Firewall                                ║
║   10. Setup Fail2ban Lite                               ║
║                                                          ║
║  ⚙️  SYSTEM                                              ║
║   11. Disable KDE Screensaver                           ║
║   12. Set UK Locale                                     ║
║   13. Mount DATA Partition                              ║
║   14. Configure Graphics                                ║
║   15. Configure Audio                                   ║
║                                                          ║
║  🔄 AUTOMATION                                           ║
║   16. Run All Bootstrap Steps                           ║
║                                                          ║
║   Q. Quit                                               ║
║                                                          ║
╚══════════════════════════════════════════════════════════╝

Enter choice:
```

## Modules

### 1. Package Installation

#### Core Packages (`13_packages_core.sh`)

Installs essential system tools:

```bash
# Text editors
vim, nano, emacs

# System monitoring
htop, btop, iotop, nethogs

# Development basics
git, gcc, make, cmake

# Network tools
curl, wget, rsync, nc

# Utilities
tmux, screen, tree, lsof, strace

# Archiving
tar, gzip, bzip2, xz, zip, unzip
```

**Usage**:
```bash
/data/mash-staging/helpers/13_packages_core.sh
```

#### Development Tools (`14_packages_dev.sh`)

Installs comprehensive development environment:

```bash
# Compilers & Languages
gcc, g++, clang
rust, cargo
golang
python3-devel, python3-pip
nodejs, npm

# Build systems
cmake, ninja-build, meson

# Version control
git, git-lfs, mercurial

# Debugging
gdb, valgrind, strace

# Documentation
doxygen, sphinx
```

**Usage**:
```bash
/data/mash-staging/helpers/14_packages_dev.sh
```

#### Desktop Applications (`15_packages_desktop.sh`)

Installs common desktop software:

```bash
# Office
libreoffice-calc, libreoffice-writer, libreoffice-impress

# Graphics
gimp, inkscape, krita

# Media
vlc, audacity, handbrake

# Communication
thunderbird, telegram-desktop

# Utilities
transmission, qbittorrent
keepassxc
```

**Usage**:
```bash
/data/mash-staging/helpers/15_packages_desktop.sh
```

### 2. Browser Setup

#### Brave Browser (`17_brave_browser.sh`)

Installs Brave browser with proper RPM signing:

```bash
# Adds Brave repository
# Imports GPG key
# Installs brave-browser package
# Configures system integration
```

**Usage**:
```bash
/data/mash-staging/helpers/17_brave_browser.sh
```

#### Set Default Browser (`17_brave_default.sh`)

Sets Brave as the system default browser:

```bash
xdg-settings set default-web-browser brave-browser.desktop
```

### 3. Hardware Configuration

#### Argon ONE Fan Control (`20_argon_one.sh`)

Configures the Argon ONE case fan and power button:

```bash
# Installs argonone daemon
# Configures temperature thresholds:
#   55°C → 10% fan speed
#   60°C → 55% fan speed
#   65°C → 100% fan speed
# Enables power button support
# Enables safe shutdown
```

**Features**:
- Temperature-based fan control
- Power button functionality (double-tap to shutdown)
- I2C communication with case controller
- Systemd service integration

**Usage**:
```bash
/data/mash-staging/helpers/20_argon_one.sh
```

### 4. Shell Customization

#### ZSH + Starship (`21_zsh_starship.sh`)

Sets up modern shell environment:

```bash
# Installs ZSH
# Installs Starship prompt
# Configures ~/.zshrc
# Sets ZSH as default shell
# Applies custom Starship theme
```

**Features**:
- Fast, modern prompt
- Git integration
- Command timing
- Directory truncation
- Error indicators
- Package manager integration

**Usage**:
```bash
/data/mash-staging/helpers/21_zsh_starship.sh
```

### 5. Snapshot Management

#### Snapper Initialization (`11_snapper_init.sh`)

Configures automatic btrfs snapshots:

```bash
# Creates snapper config for /
# Sets up hourly snapshots
# Configures retention:
#   - Hourly: keep 10
#   - Daily: keep 7
#   - Weekly: keep 4
#   - Monthly: keep 3
# Enables timeline snapshots
```

**Usage**:
```bash
/data/mash-staging/helpers/11_snapper_init.sh
```

**Commands**:
```bash
# List snapshots
snapper list

# Create manual snapshot
snapper create --description "Before update"

# Rollback to snapshot
snapper rollback <number>
```

### 6. Security

#### Firewall Configuration (`12_firewall_sane.sh`)

Configures firewalld with sensible defaults:

```bash
# Enables firewalld
# Sets default zone to 'public'
# Opens SSH (port 22)
# Blocks all other incoming by default
# Allows all outgoing
```

**Usage**:
```bash
/data/mash-staging/helpers/12_firewall_sane.sh
```

#### Fail2ban Lite (`03_fail2ban_lite.sh`)

Sets up basic fail2ban protection:

```bash
# Installs fail2ban
# Configures SSH jail
# Sets ban time: 10 minutes
# Max retries: 5
# Find time: 10 minutes
```

**Usage**:
```bash
/data/mash-staging/helpers/03_fail2ban_lite.sh
```

### 7. System Tweaks

#### Disable KDE Screensaver (`22_kde_screensaver_nuke.sh`)

Removes KDE lockscreen and screensaver:

```bash
# Disables screen locking
# Removes screensaver timeout
# Prevents screen blanking
# Disables DPMS
```

**Usage**:
```bash
/data/mash-staging/helpers/22_kde_screensaver_nuke.sh
```

#### UK Locale (`10_locale_uk.sh`)

Sets UK locale and keyboard:

```bash
# Sets locale to en_GB.UTF-8
# Configures keyboard layout to 'gb'
# Updates system settings
```

**Usage**:
```bash
/data/mash-staging/helpers/10_locale_uk.sh
```

#### Mount DATA Partition (`16_mount_data.sh`)

Ensures DATA partition is mounted:

```bash
# Creates /data mount point
# Adds entry to /etc/fstab
# Mounts LABEL=DATA to /data
# Verifies mount
```

**Usage**:
```bash
/data/mash-staging/helpers/16_mount_data.sh
```

## Early Boot Services

### Early SSH (`mash-early-ssh.service`)

Starts SSH server as soon as network is available:

```ini
[Unit]
Description=MASH Early SSH
After=network-online.target
Wants=network-online.target

[Service]
Type=oneshot
ExecStart=/usr/local/lib/mash/system/early-ssh.sh
RemainAfterExit=yes

[Install]
WantedBy=multi-user.target
```

**Features**:
- Starts before most other services
- Enables remote access quickly
- Useful for headless setups
- Waits for network connectivity

### Internet Wait (`mash-internet-wait.service`)

Waits for internet connectivity before proceeding:

```ini
[Unit]
Description=MASH Internet Wait
After=network-online.target
Before=dnf-makecache.service

[Service]
Type=oneshot
ExecStart=/usr/local/lib/mash/system/internet-wait.sh
RemainAfterExit=yes

[Install]
WantedBy=multi-user.target
```

**Features**:
- Pings 8.8.8.8 until successful
- Blocks dependent services
- Prevents premature package updates
- Configurable timeout

## Bootstrap Runner

The bootstrap runner (`bootstrap.sh`) orchestrates running multiple helper scripts in sequence:

```bash
#!/usr/bin/env bash
# Run all bootstrap steps

HELPERS="/data/mash-staging/helpers"

# Run in order
for script in \
    00_write_config_txt.sh \
    02_early_ssh.sh \
    03_fail2ban_lite.sh \
    10_locale_uk.sh \
    11_snapper_init.sh \
    12_firewall_sane.sh \
    13_packages_core.sh \
    16_mount_data.sh \
    20_argon_one.sh \
    21_zsh_starship.sh \
    22_kde_screensaver_nuke.sh
do
    if [ -f "$HELPERS/$script" ]; then
        echo "Running $script..."
        bash "$HELPERS/$script"
    fi
done

echo "Bootstrap complete!"
```

## Customization

### Adding New Modules

1. Create script in `/data/mash-staging/helpers/`:
```bash
#!/usr/bin/env bash
# helpers/30_my_custom_fix.sh

echo "Applying my custom fix..."
# Your code here
```

2. Make executable:
```bash
chmod +x helpers/30_my_custom_fix.sh
```

3. Add to Dojo menu in `dojo/usr_local_lib_mash/dojo/dojo.sh`

4. Add to bootstrap runner if needed

### Modifying Existing Modules

Edit the helper script directly:
```bash
vim /data/mash-staging/helpers/13_packages_core.sh
```

Changes take effect immediately on next run.

## Troubleshooting

### Dojo Not Appearing

```bash
# Check autostart file
cat /etc/xdg/autostart/mash-dojo.desktop

# Launch manually
/usr/local/bin/mash-dojo-launch

# Reinstall
sudo /data/mash-staging/install_dojo.sh /data/mash-staging
```

### Helper Scripts Failing

```bash
# Check logs
sudo journalctl -u mash-early-ssh
sudo journalctl -u mash-internet-wait

# Run manually with debug
bash -x /data/mash-staging/helpers/13_packages_core.sh
```

### Early SSH Not Working

```bash
# Check service status
sudo systemctl status mash-early-ssh

# Restart service
sudo systemctl restart mash-early-ssh

# Check script
sudo /usr/local/lib/mash/system/early-ssh.sh
```

### Package Installation Fails

```bash
# Update package cache
sudo dnf clean all
sudo dnf makecache

# Check internet
ping -c 4 8.8.8.8

# Run internet wait
sudo /usr/local/lib/mash/system/internet-wait.sh
```

## Best Practices

1. **Run Bootstrap Once**: The bootstrap is designed to run once after first boot
2. **Check Dependencies**: Some helpers depend on others (e.g., Brave needs repositories)
3. **Use Snapper**: Take snapshots before major changes
4. **Keep Staged**: Don't delete `/data/mash-staging/` - useful for repairs
5. **Log Everything**: Check `/data/mash-logs/` for boot logs

## Advanced Usage

### Scripted Installation

```bash
# Run specific modules only
for module in 13 14 20 21; do
    bash /data/mash-staging/helpers/${module}_*.sh
done
```

### Remote Bootstrap

```bash
# SSH in and run bootstrap
ssh pi@192.168.1.100
sudo /usr/local/lib/mash/dojo/bootstrap.sh
```

### Automated Post-Install

Add to `/data/mash-staging/autorun.sh`:
```bash
#!/usr/bin/env bash
# Auto-run specific helpers
bash /data/mash-staging/helpers/13_packages_core.sh
bash /data/mash-staging/helpers/20_argon_one.sh
```

## Reference

### Helper Script Naming Convention

```
XX_description.sh
│   │
│   └── Descriptive name
└────── Order number (00-99)
```

**Number Ranges**:
- 00-09: Initial setup & configuration
- 10-19: System settings (locale, snapper, firewall)
- 20-29: Hardware configuration
- 30-39: Reserved for custom scripts
- 40+: Advanced features

### Exit Codes

All helper scripts use standard exit codes:
- `0`: Success
- `1`: General error
- `2`: Missing dependency
- `3`: Permission denied
- `4`: Network error

## Contributing

To add new Dojo modules:

1. Create helper script in `helpers/`
2. Add module in `dojo/usr_local_lib_mash/dojo/`
3. Update menu in `dojo.sh`
4. Test thoroughly
5. Document in this file
6. Submit PR

---

**The Dojo way: Automate wisely, configure precisely, enjoy completely. 🥋**
