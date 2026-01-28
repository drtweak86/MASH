# MASH Installer - Complete Setup and Deployment Guide

## 📦 What You Have

This complete package includes:

```
mash-complete/
├── .github/workflows/build.yml    # CI/CD automation
├── mash-installer/                # Rust CLI
│   ├── src/
│   │   ├── main.rs               # Entry point
│   │   ├── cli.rs                # Argument parsing
│   │   ├── flash.rs              # Core installation logic ⭐
│   │   ├── preflight.rs          # System checks
│   │   ├── errors.rs             # Error handling
│   │   └── logging.rs            # Logging setup
│   └── Cargo.toml                # Dependencies
├── qt-gui/                        # Qt GUI wrapper
│   ├── src/
│   │   ├── main.cpp
│   │   ├── mainwindow.cpp/.h     # Main window logic
│   │   └── mainwindow.ui         # UI layout
│   ├── CMakeLists.txt
│   └── mash-installer.desktop.in
├── scripts/
│   └── bump-version.sh           # Version management
├── docs/
│   ├── ARCHITECTURE.md           # Technical details
│   └── QUICKSTART.md             # User guide
├── install.sh                     # One-command installer
├── Makefile                       # Build system
├── README.md                      # Main documentation
├── LICENSE                        # MIT License
└── .gitignore
```

## 🚀 Deployment Steps

### Step 1: Set Up GitHub Repository

```bash
# Extract the archive
tar -xzf mash-complete.tar.gz
cd mash-complete

# Initialize git (if not already done)
git init
git add .
git commit -m "Initial commit: MASH Installer v0.3.0"

# Create GitHub repository (via web or gh CLI)
gh repo create mash-installer --public --source=. --remote=origin --push

# Or manually:
git remote add origin https://github.com/drtweak86/MASH.git
git branch -M main
git push -u origin main
```

### Step 2: Update Repository-Specific Values

Edit these files to match your repository:

**1. install.sh** (Line 13)
```bash
REPO="drtweak86/MASH"  # ← Change this!
```

**2. README.md**
Replace all instances of `drtweak86/MASH` with your actual repository.

**3. .github/workflows/build.yml**
The workflow uses `${{ github.repository }}` so it's automatically correct.

### Step 3: Enable GitHub Actions

1. Go to your repository on GitHub
2. Click "Actions" tab
3. Enable workflows if prompted
4. GitHub Actions will automatically build on push

### Step 4: Create First Release

```bash
# Make sure everything is updated
git add -A
git commit -m "Update repository URLs"
git push

# Create first release
./scripts/bump-version.sh patch

# Push tags to trigger release
git push origin main --tags
```

This will:
- Build binaries for ARM64 and x86_64
- Create Qt GUI executable
- Package everything into a release
- Make available via GitHub Releases

### Step 5: Update Install Script URL

After first release, update your documentation to use:
```bash
curl -fsSL https://raw.githubusercontent.com/drtweak86/MASH/main/install.sh | sudo bash
```

## 🔧 Local Development

### Build Locally

```bash
# Install dependencies (Fedora)
sudo dnf install -y rust cargo cmake qt6-qtbase-devel

# Or (Ubuntu/Debian)
sudo apt install -y cargo cmake qt6-base-dev

# Build everything
make

# Or build individually
make build-cli   # Rust CLI only
make build-qt    # Qt GUI only

# Run tests
make test

# Install system-wide
sudo make install
```

### Development Workflow

```bash
# 1. Make changes to source files

# 2. Test locally
cd mash-installer
cargo build
cargo test

# 3. Test CLI
RUST_LOG=debug cargo run -- preflight

# 4. Test GUI
cd ../qt-gui
cmake -B build && cmake --build build
./build/mash-installer-qt

# 5. Commit changes
git add .
git commit -m "Add new feature"
git push
```

### Version Bumping

```bash
# Patch version (0.3.0 → 0.3.1)
./scripts/bump-version.sh patch

# Minor version (0.3.0 → 0.4.0)
./scripts/bump-version.sh minor

# Major version (0.3.0 → 1.0.0)
./scripts/bump-version.sh major

# Push to trigger release
git push origin main --tags
```

## 📋 Key Features Implemented

### ✅ Core Installation (`flash.rs`)

- **Loop Mount Support**: Mounts raw images without extraction
- **MBR Partitioning**: 4 partitions (EFI, BOOT, ROOT, DATA)
- **Smart Formatting**: FAT32 for EFI, ext4 for others
- **rsync Copy**: Efficient system copy with exclusions
- **UEFI Configuration**: Dracut + GRUB installation
- **UUID-based fstab**: Proper disk identification
- **Safety Features**: Dry-run, confirmations, lsblk display

### ✅ Qt GUI

- **File Browser**: Easy image selection
- **Disk Detection**: Auto-discover available disks
- **Real-time Logging**: Color-coded output
- **Progress Bar**: Visual feedback
- **Safety Dialogs**: Multiple confirmations
- **pkexec Integration**: Privilege elevation

### ✅ CI/CD Pipeline

- **Multi-Architecture**: ARM64 + x86_64
- **Automated Builds**: On every push
- **Release Automation**: On git tags
- **Artifact Management**: Downloadable binaries
- **Checksums**: SHA256 verification

### ✅ One-Command Install

- **Architecture Detection**: Auto-selects correct binary
- **Version Detection**: Always gets latest release
- **Desktop Integration**: Creates application menu entry
- **Clean Installation**: Single command setup

## 🎯 Usage Examples

### CLI Examples

```bash
# Check system
mash-installer preflight

# Dry run (safe test)
sudo mash-installer flash \
  --image ~/fedora.raw \
  --disk /dev/sda \
  --uefi-dir ~/uefi \
  --dry-run

# Real installation
sudo mash-installer flash \
  --image ~/Fedora-KDE-40.raw \
  --disk /dev/sda \
  --uefi-dir ~/rpi4-uefi \
  --auto-unmount \
  --yes-i-know

# With debugging
RUST_LOG=debug sudo mash-installer flash [options]
```

### GUI Usage

```bash
# Launch GUI
sudo mash-installer-qt

# Steps in GUI:
# 1. Browse for Fedora image
# 2. Select target disk from dropdown
# 3. Set UEFI directory (or use default)
# 4. Check "Dry Run" to test (optional)
# 5. Click "Install"
# 6. Confirm warnings
# 7. Wait for completion
```

## 🔒 Security Considerations

### Safety Mechanisms

1. **`--yes-i-know` flag**: Required for destructive operations
2. **Disk verification**: Shows lsblk before proceeding
3. **Dry-run mode**: Test without making changes
4. **Confirmation prompts**: CLI asks before critical steps
5. **Double confirmation**: GUI requires two confirmations
6. **pkexec**: Proper privilege escalation

### Best Practices

- Always test with `--dry-run` first
- Double-check disk selection with `lsblk`
- Keep backups of important data
- Verify UEFI firmware version compatibility
- Test on spare hardware first

## 📊 Partition Layout Explained

```
/dev/sda (or /dev/mmcblk0)
├─ sda1: EFI     512 MB   FAT32   boot flag
│  └─ /boot/efi
├─ sda2: BOOT    1 GB     ext4
│  └─ /boot
├─ sda3: ROOT    1.8 TB   ext4
│  └─ /
└─ sda4: DATA    ~1.9 TB  ext4
   └─ /data
```

### Why This Layout?

- **EFI**: UEFI firmware files, must be FAT32
- **BOOT**: Kernel, initramfs, separate for easier updates
- **ROOT**: System files, 1.8TB for software and cache
- **DATA**: User data, persistent across reinstalls

## 🛠️ Customization

### Modify Partition Sizes

Edit `mash-installer/src/flash.rs`:

```rust
const EFI_SIZE_MB: u64 = 512;      // Change this
const BOOT_SIZE_MB: u64 = 1024;    // Change this
const ROOT_SIZE_GB: u64 = 1800;    // Change this
// DATA uses remaining space automatically
```

### Add Custom Post-Install Scripts

Edit `post_install_fixes()` in `flash.rs`:

```rust
fn post_install_fixes(disk: &str) -> Result<()> {
    // Your custom steps here
    
    // Example: Install additional packages
    Command::new("sudo")
        .args(["chroot", root_mount.to_str().unwrap(),
               "dnf", "install", "-y", "vim", "htop"])
        .status()?;
    
    // Your code...
}
```

### Modify UEFI Configuration

Edit `configure_uefi()` in `flash.rs` to adjust:
- GRUB configuration
- Dracut options
- Kernel parameters
- Boot timeout

## 🐛 Troubleshooting

### Build Issues

**Rust not found:**
```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source $HOME/.cargo/env
```

**Qt not found:**
```bash
# Fedora
sudo dnf install qt6-qtbase-devel

# Ubuntu
sudo apt install qt6-base-dev
```

### Runtime Issues

**Permission denied:**
```bash
# Make sure you're using sudo
sudo mash-installer [command]
```

**Loop device busy:**
```bash
# Check for stale loop devices
sudo losetup -a

# Detach if needed
sudo losetup -d /dev/loop0
```

**Partition already mounted:**
```bash
# Use --auto-unmount flag
sudo mash-installer flash --auto-unmount [options]
```

## 📖 Documentation

- **README.md**: Overview and quick start
- **docs/ARCHITECTURE.md**: Technical deep dive
- **docs/QUICKSTART.md**: Step-by-step guide
- **Source comments**: Inline documentation

## 🤝 Contributing

We welcome contributions! Here's how:

1. Fork the repository
2. Create feature branch: `git checkout -b feature/amazing`
3. Make your changes
4. Add tests if applicable
5. Run `cargo fmt` and `cargo clippy`
6. Commit: `git commit -m 'Add amazing feature'`
7. Push: `git push origin feature/amazing`
8. Open Pull Request

## 📝 License

MIT License - see LICENSE file for details.

## 🙏 Acknowledgments

- Rust community for amazing tools
- Qt Project for cross-platform framework
- Fedora Project for ARM support
- Raspberry Pi Foundation
- GitHub for free CI/CD

## 📬 Support

- **Issues**: https://github.com/drtweak86/MASH/issues
- **Discussions**: https://github.com/drtweak86/MASH/discussions
- **Email**: your-email@example.com

---

**Happy Installing! 🎉**

Made with ❤️ and 🦀 Rust
