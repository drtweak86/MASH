# MASH Installer Architecture

## Overview

MASH Installer is a modular system for deploying Fedora KDE on Raspberry Pi 4B with UEFI boot. It consists of:

1. **Rust CLI** - Core installation logic
2. **GUI (optional)** - User-friendly interface
3. **CI/CD Pipeline** - Automated builds and releases
4. **Install Script** - One-command deployment

## Components

### 1. Rust CLI (`mash-installer/`)

#### Module Structure

```
src/
├── main.rs          - Entry point, command routing
├── cli.rs           - Argument parsing with clap
├── preflight.rs     - System validation checks
├── flash.rs         - Core installation pipeline
├── errors.rs        - Custom error types
└── logging.rs       - Logging configuration
```

#### Key Features

- **Loop Mount Handling**: Mounts raw images using `losetup`
- **Partition Management**: Creates MBR with 4 partitions using `parted`
- **rsync System Copy**: Efficiently copies filesystem
- **UEFI Configuration**: Runs `dracut` and installs GRUB
- **Safety Features**: Dry-run mode, confirmation prompts, UUID-based fstab

### 2. GUI (optional) (`GUI/`)

#### Components

```
src/
├── main.cpp         - GUI (optional, x86_64 only) application entry
├── mainwindow.h     - Window class declaration
├── mainwindow.cpp   - Window implementation
└── mainwindow.ui    - UI layout (GUI (optional, x86_64 only) Designer)
```

#### Features

- **Process Management**: Spawns and monitors CLI process
- **Real-time Logging**: Captures stdout/stderr
- **Progress Indication**: Visual feedback during installation
- **Safety Checks**: Validation before destructive operations
- **Disk Detection**: Auto-discovery of available disks

### 3. GitHub Actions (`.github/workflows/`)

#### Build Matrix

```yaml
Strategy:
  - aarch64-unknown-linux-gnu  # For Raspberry Pi 4
  - x86_64-unknown-linux-gnu   # For host systems
```

#### Workflow Steps

1. **Build Rust CLI**
   - Cross-compile for ARM64 and x86_64
   - Strip binaries for size optimization
   - Upload as artifacts

2. **Build GUI (optional)**
   - Compile with GUI (optional, x86_64 only) 6
   - Create single-file executable
   - Upload as artifact

3. **Create Release** (on tag)
   - Download all artifacts
   - Create release tarball
   - Generate checksums
   - Publish GitHub release

4. **Auto-Release** (on version bump)
   - Detect version changes in Cargo.toml
   - Create and push git tag
   - Trigger release workflow

## Installation Pipeline

### Phase 1: Preflight

```rust
preflight::run()
├── Check if running as root
├── Verify required tools (parted, mkfs, rsync, losetup, dracut)
├── Check disk availability
└── Validate image file
```

### Phase 2: Disk Preparation

```rust
partition_disk()
├── wipefs -a /dev/sdX          # Wipe existing data
├── parted mklabel msdos        # Create MBR
├── Create partition 1: EFI     # 512MB, FAT32
├── Create partition 2: BOOT    # 1GB, ext4
├── Create partition 3: ROOT    # 1.8TB, ext4
├── Create partition 4: DATA    # Remaining, ext4
└── parted set 1 boot on        # Set boot flag
```

### Phase 3: Formatting

```rust
format_partitions()
├── mkfs.vfat -F 32 /dev/sdX1   # EFI
├── mkfs.ext4 /dev/sdX2         # BOOT
├── mkfs.ext4 /dev/sdX3         # ROOT
└── mkfs.ext4 /dev/sdX4         # DATA
```

### Phase 4: System Installation

```rust
install_system()
├── losetup -f --show -P image.raw     # Setup loop device
├── mount /dev/loopXpY /tmp/loop       # Mount image
├── mount /dev/sdX3 /tmp/root          # Mount target
├── rsync -aAXHv /tmp/loop/ /tmp/root/ # Copy system
├── umount /tmp/loop                   # Cleanup
└── losetup -d /dev/loopX             # Detach loop
```

### Phase 5: UEFI Configuration

```rust
configure_uefi()
├── mount /dev/sdX2 /mnt/boot          # Mount BOOT
├── mount /dev/sdX1 /mnt/boot/efi      # Mount EFI
├── Copy UEFI firmware files
├── Get UUIDs with blkid
├── Generate /etc/fstab with UUIDs
├── chroot into system
├── dracut --force                     # Generate initramfs
├── grub2-mkconfig                     # Generate GRUB config
├── grub2-install --target=arm64-efi   # Install GRUB
└── umount all
```

### Phase 6: Post-Installation

```rust
post_install_fixes()
├── Enable NetworkManager
├── Enable SDDM (KDE display manager)
├── Enable Bluetooth
├── Create /data mount point
└── Final cleanup
```

## Data Flow

```
User Input (CLI/GUI)
    ↓
Argument Parsing (clap)
    ↓
Validation (preflight)
    ↓
Disk Operations (flash)
    ├→ Partition (parted)
    ├→ Format (mkfs)
    ├→ Mount (loop + target)
    ├→ Copy (rsync)
    ├→ UEFI (dracut, grub)
    └→ Cleanup
    ↓
Success/Error Result
    ↓
User Feedback
```

## Error Handling

### Error Types

```rust
enum MashError {
    MissingYesIKnow,      // Safety flag not provided
    Aborted,              // User cancelled
    CommandFailed(String), // External command error
    InvalidImage(String),  // Bad image file
    DiskNotFound(String),  // Disk doesn't exist
    InsufficientSpace,    // Not enough space
    PartitionError(String), // Partition operation failed
    MountError(String),    // Mount operation failed
    UefiError(String),     // UEFI config failed
}
```

### Error Propagation

```rust
main() -> Result<()>
    ↓
flash::run() -> Result<()>
    ↓ (uses ? operator)
partition_disk() -> Result<()>
    ↓ (uses .context())
Command::new().status() -> Result<()>
```

## Security Considerations

1. **Root Requirements**: Most operations require root privileges
2. **Safety Flags**: `--yes-i-know` prevents accidental data loss
3. **Disk Verification**: Shows lsblk output before operations
4. **Dry-run Mode**: Test without making changes
5. **pkexec Integration**: GUI uses PolicyKit for privilege elevation

## Performance Optimizations

1. **Parallel Builds**: GitHub Actions uses build matrix
2. **LTO**: Link-time optimization for smaller binaries
3. **Strip Binaries**: Remove debug symbols
4. **rsync Efficiency**: Uses archive mode with hard links
5. **Caching**: GitHub Actions caches Cargo dependencies

## Cross-Platform Support

### Build Targets

- **ARM64**: Native execution on Raspberry Pi 4
- **x86_64**: For building images on desktop/laptop

### Cross-Compilation

```bash
# Install cross-compiler
apt-get install gcc-aarch64-linux-gnu

# Build for ARM64
export CARGO_TARGET_AARCH64_UNKNOWN_LINUX_GNU_LINKER=aarch64-linux-gnu-gcc
cargo build --target aarch64-unknown-linux-gnu --release
```

## Testing Strategy

### Unit Tests

```rust
#[cfg(test)]
mod tests {
    #[test]
    fn test_normalize_disk() { }
    
    #[test]
    fn test_get_uuid() { }
}
```

### Integration Tests

```bash
# Dry-run test
make preflight
mash-installer flash --dry-run [options]
```

### CI Tests

- Compile check for all targets
- Run unit tests
- Build artifacts validation

## Future Enhancements

### Planned Features

1. **Progress Reporting**: Real-time progress percentage
2. **Incremental Updates**: Update existing installations
3. **Backup/Restore**: Backup before installation
4. **Multiple Distros**: Support other ARM distros
5. **Web UI**: Browser-based installer
6. **Image Builder**: Build custom images

### Architectural Improvements

1. **Plugin System**: Modular distro support
2. **State Machine**: Better error recovery
3. **Async Operations**: Non-blocking I/O
4. **Database**: Track installations
5. **Logging Backend**: Remote logging support

## Dependencies

### Rust Crates

- `clap`: CLI argument parsing
- `anyhow`: Error handling
- `log` + `env_logger`: Logging
- `thiserror`: Custom errors
- `serde`: Serialization (future)
- `nix`: Unix system calls (future)

### System Dependencies

- `parted`: Partition management
- `mkfs.vfat`, `mkfs.ext4`: Filesystem creation
- `rsync`: File copying
- `losetup`: Loop device management
- `dracut`: Initramfs generation
- `grub2-install`: Bootloader installation
- `blkid`: UUID detection
- `lsblk`: Disk information

### Build Dependencies

- Rust 1.70+
- GUI (optional, x86_64 only) 5.15+ or GUI (optional, x86_64 only) 6.x
- CMake 3.16+
- GCC/G++ (with ARM cross-compiler for ARM builds)

## References

- [Fedora ARM Documentation](https://fedoraproject.org/wiki/Architectures/ARM)
- [RPi4 UEFI Firmware](https://github.com/pftf/RPi4)
- [Rust Book](https://doc.rust-lang.org/book/)
- [GUI (optional, x86_64 only) Documentation](https://doc.qt.io/)
- [GitHub Actions](https://docs.github.com/en/actions)
