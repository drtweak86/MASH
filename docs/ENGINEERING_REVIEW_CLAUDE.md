# Grand Improvement & Refactoring Plan
## Third-Party Engineering Review by Claude (French)

**Repository**: MASH (Minimal, Automated, Self-Hosting)
**Version Reviewed**: v1.2.14
**Date**: 2026-02-04
**Scope**: All code excluding `legacy/` and `legacy_scripts/`

---

# A) Executive Summary

## What's Good
- **Type-safe safety model**: `ExecuteArmToken` + typestate pattern prevents accidental destructive ops
- **HAL abstraction**: Clean trait separation (`MountOps`, `FormatOps`, etc.) enables testable mocks
- **Timeout enforcement**: All external commands have explicit timeouts with graceful kill
- **Concurrent pipe draining**: Prevents deadlocks on chatty stderr/stdout
- **Resume capability**: Checkpoint-based state manager allows crash recovery
- **Dry-run pervasive**: Every destructive path respects `dry_run` flag

## What's Risky
- **14 external command dependencies**: `mkfs.*`, `parted`, `rsync`, `losetup`, `blkid`, `lsblk`, `wipefs`, `btrfs`, `sync`, `udevadm` — any missing binary = runtime failure
- **No idempotency guards**: Re-running mount/format without state checks can fail or corrupt
- **TUI monolith**: 2,652-line `dojo_app.rs` with interleaved state/render/input logic
- **Error type duplication**: `HalError` and `MashError` overlap with identical variants
- **Pi 1GB RAM**: No memory pressure handling; large rsync buffers could OOM

## What Must Change for Production-Grade One-Shot
1. **Replace external commands with Rust syscalls/crates** — eliminate runtime dependency on 14 binaries
2. **Add idempotency guards** — check "already mounted/formatted" before acting
3. **RAII cleanup guards** — ensure mounts/loops detach even on panic
4. **Memory-bounded streaming** — cap buffer sizes for 1GB Pi environments
5. **Split TUI monolith** — separate state machine, renderer, input handler

---

# B) Current Architecture Map

## Workspace Crates

```
mash-installer/          # Entry point (212 LOC) — CLI dispatch
├── mash-core/           # Core library (4,389 LOC)
│   ├── downloader/      # HTTP fetch, resume, checksum
│   ├── state_manager/   # Checkpoint persistence (JSON)
│   ├── boot_config/     # UEFI/kernel cmdline patching
│   ├── system_config/   # Package/service management
│   ├── locale.rs        # Locale file patching
│   └── flash.rs         # Image → disk write orchestration
├── crates/
│   ├── mash-hal/        # Hardware Abstraction Layer (577 LOC traits + 713 LOC impl)
│   │   ├── linux_hal.rs # Real system: Command::spawn + nix syscalls
│   │   └── fake_hal.rs  # Mock for CI (523 LOC)
│   ├── mash-tui/        # Terminal UI (3,839 LOC)
│   │   └── dojo/        # Main TUI app (2,652 LOC dojo_app.rs)
│   └── mash-workflow/   # Pipeline orchestration (1,088 LOC)
│       ├── pipeline.rs  # Stage builder and execution
│       └── preflight.rs # System readiness checks
└── libdnf-sys/          # Optional FFI to libdnf5 (Fedora packages)
```

## Key Data Flows

```
┌─────────────────────────────────────────────────────────────────┐
│ 1. DOWNLOAD                                                     │
│    URL → reqwest → XzDecoder → temp file → SHA256 verify        │
├─────────────────────────────────────────────────────────────────┤
│ 2. FLASH (raw image mode)                                       │
│    .img.xz → XzDecoder → io::copy → /dev/sdX                    │
├─────────────────────────────────────────────────────────────────┤
│ 3. PARTITION                                                    │
│    wipefs -a → parted mklabel → parted mkpart (×N) → udev settle│
├─────────────────────────────────────────────────────────────────┤
│ 4. FORMAT                                                       │
│    mkfs.vfat (EFI) → mkfs.ext4 (boot) → mkfs.btrfs (root)       │
├─────────────────────────────────────────────────────────────────┤
│ 5. MOUNT + COPY                                                 │
│    nix::mount → rsync -aHAX → sync                              │
├─────────────────────────────────────────────────────────────────┤
│ 6. BOOT CONFIG                                                  │
│    fstab generation → cmdline patching → UEFI entries           │
├─────────────────────────────────────────────────────────────────┤
│ 7. CLEANUP                                                      │
│    unmount_recursive → losetup -d → state persist               │
└─────────────────────────────────────────────────────────────────┘
```

## State Storage & Mutation

| Location | Format | Contents |
|----------|--------|----------|
| `~/.mash/state.json` | JSON | `InstallState`: completed stages, download artifacts, checksums |
| `/proc/self/mountinfo` | Kernel | Live mount table (read-only) |
| `/sys/block/*/` | Kernel | Block device metadata (read-only) |
| TUI `DojoApp` struct | In-memory | Screen state, selections, progress channels |

---

# C) Findings by Category

## 1. Safety & Destructive Ops

| Finding | Impact | Evidence | Recommendation |
|---------|--------|----------|----------------|
| **GOOD**: Two-factor arming (`DESTROY` + typestate token) | High | `config_states.rs:ExecuteArmToken` | Keep as-is |
| **GOOD**: Boot disk detection + special confirmation | High | `dojo_app.rs:928-930` | Keep as-is |
| **GAP**: No confirmation on individual partition ops | Medium | `parted()` only checks `opts.confirmed` once | Add per-partition confirmation or summary |
| **GAP**: `format_*` silently proceeds if confirmed=true | Low | Caller must ensure safety; no runtime re-check | Document invariant or add re-verification |

## 2. Idempotency & Resume

| Finding | Impact | Evidence | Recommendation |
|---------|--------|----------|----------------|
| **GOOD**: Stage checkpoint system | High | `state_manager/mod.rs` | Keep as-is |
| **GAP**: Mount fails if already mounted | Medium | `nix::mount` returns EBUSY | Add `is_mounted()` guard |
| **GAP**: Format succeeds even if FS exists and matches | Low | `mkfs.ext4` overwrites unconditionally | Add FS type check before format |
| **GAP**: rsync doesn't detect "already complete" | Low | Always re-runs full copy | Add `--checksum` or size comparison |

**Before/After Example — Idempotency Guard**:
```rust
// BEFORE (linux_hal.rs:116)
fn mount_device(&self, device: &Path, target: &Path, ...) -> HalResult<()> {
    nix::mount::mount(Some(device), target, fstype, flags, data)
        .map_err(map_nix_err)?;  // Fails with EBUSY if already mounted
    Ok(())
}

// AFTER
fn mount_device(&self, device: &Path, target: &Path, ...) -> HalResult<()> {
    if self.is_mounted(target)? {
        log::debug!("{} already mounted, skipping", target.display());
        return Ok(());
    }
    nix::mount::mount(Some(device), target, fstype, flags, data)
        .map_err(map_nix_err)?;
    Ok(())
}
```

## 3. Disk Identity & Source-Disk Protection

| Finding | Impact | Evidence | Recommendation |
|---------|--------|----------|----------------|
| **GOOD**: Boot disk detection via `/proc/self/mountinfo` | High | `procfs/mountinfo.rs` | Keep as-is |
| **GOOD**: Visual warning for source disk selection | High | `DiskOption.is_source_disk` | Keep as-is |
| **GAP**: Disk identity by path (`/dev/sda`) not stable | Medium | USB re-enumeration can shuffle letters | Use `/dev/disk/by-id/` or serial |
| **GAP**: No SCSI/USB serial verification before flash | Low | User could swap disks between selection and execute | Add serial re-check at flash time |

## 4. Performance & Memory (Pi 1GB Focus)

| Finding | Impact | Evidence | Recommendation |
|---------|--------|----------|----------------|
| **RISK**: rsync buffers unbounded | High | `BufReader::new(stdout)` default 8KB, but channel unbounded | Use bounded channel (32-64 items) |
| **RISK**: Download to temp file, then copy to disk | Medium | Doubles I/O; image sits in page cache | Stream directly with tee pattern |
| **RISK**: XZ decompression in-memory | Medium | Large .xz files expand in RAM | Verify `xz2::XzDecoder` streams (it does) |
| **OK**: Release profile uses LTO + strip | N/A | `Cargo.toml` profile.release | Keep as-is |

**Performance Hotspot Example**:
```rust
// Location: linux_hal.rs:548
let (tx, rx) = mpsc::channel::<io::Result<String>>();  // UNBOUNDED

// Recommendation: Use bounded channel to apply backpressure
let (tx, rx) = mpsc::sync_channel::<io::Result<String>>(64);
```

## 5. Error Handling & Typed Errors

| Finding | Impact | Evidence | Recommendation |
|---------|--------|----------|----------------|
| **GAP**: Duplicate error enums | Medium | `HalError` and `MashError` both have `SafetyLock`, `DiskBusy`, etc. | Unify into single hierarchy |
| **GAP**: 154 `unwrap()`/`expect()` calls | High | Panic in privileged context = bad | Replace with `?` + typed errors |
| **GAP**: Some errors use `String` payload | Low | `HalError::Other(String)` loses type safety | Add specific variants |

**Before/After Example — Typed Error**:
```rust
// BEFORE (downloader/mod.rs)
return Err(anyhow!("checksum mismatch: {} != {}", computed, expected));

// AFTER
#[derive(Error, Debug)]
pub enum DownloadError {
    #[error("Checksum mismatch: computed {computed}, expected {expected}")]
    ChecksumMismatch { computed: String, expected: String },
    // ...
}
return Err(DownloadError::ChecksumMismatch { computed, expected }.into());
```

## 6. Logging / TUI Stability

| Finding | Impact | Evidence | Recommendation |
|---------|--------|----------|----------------|
| **GOOD**: Emoji-based log prefixes | N/A | `log::info!("💾 Flashing...")` | Keep as-is |
| **GAP**: No log-to-file during TUI | Medium | Logs go to stderr, which TUI captures | Add `--log-file` option |
| **GAP**: Progress events via unbounded channel | Low | Could lag TUI on slow updates | Bounded channel with drop-oldest |

## 7. Testability / CI Determinism

| Finding | Impact | Evidence | Recommendation |
|---------|--------|----------|----------------|
| **GOOD**: FakeHal for CI | High | `fake_hal.rs` records ops without executing | Keep as-is |
| **GOOD**: httpmock for download tests | High | 13 download tests with mock server | Keep as-is |
| **GAP**: No TUI integration tests | Medium | 2,652-line `dojo_app.rs` untested | Extract pure functions, unit test state transitions |
| **GAP**: Tests require network for some paths | Low | CI can flake on DNS | Add offline mode / fixtures |

## 8. API Boundaries & Crate Hygiene

| Finding | Impact | Evidence | Recommendation |
|---------|--------|----------|----------------|
| **GOOD**: Trait-based HAL | High | 8 distinct op traits | Keep as-is |
| **GAP**: `mash-core` re-exports from `mash-hal` | Low | Coupling between layers | Consider facade pattern |
| **GAP**: nix version mismatch (0.27 vs 0.29) | Low | `Cargo.toml` discrepancy | Align to 0.29 workspace-wide |

## 9. Duplication & Cleanup Opportunities

| Finding | Impact | Evidence | Recommendation |
|---------|--------|----------|----------------|
| **DUP**: Timeout helper (76 + 96 LOC) | Medium | `process_timeout.rs` vs inline in `linux_hal.rs` | Consolidate to shared module |
| **DUP**: Dry-run guard (45+ sites) | Low | `if opts.dry_run { log; return Ok(()); }` | Extract macro `dry_run_guard!` |
| **DUP**: Safety confirmation check (8 sites) | Low | `if !opts.confirmed { return Err(SafetyLock); }` | Add `opts.require_armed()?` |
| **DUP**: Mount path construction | Low | Manual `format!("{}/boot/efi", root)` | Create `MountLayout` struct |

**Before/After Example — Deduplicated Function**:
```rust
// BEFORE: Repeated in format_ext4, format_btrfs, format_vfat, wipefs_all, parted, flash_raw_image
if opts.dry_run {
    log::info!("DRY RUN: {} {}", operation, device.display());
    return Ok(());
}
if !opts.confirmed {
    return Err(HalError::SafetyLock);
}

// AFTER: Macro in mash-hal/src/macros.rs
macro_rules! destructive_guard {
    ($opts:expr, $op:expr, $device:expr) => {
        if $opts.dry_run {
            log::info!("DRY RUN: {} {}", $op, $device.display());
            return Ok(());
        }
        if !$opts.confirmed {
            return Err(HalError::SafetyLock);
        }
    };
}

// Usage:
fn format_ext4(&self, device: &Path, opts: &FormatOptions) -> HalResult<()> {
    destructive_guard!(opts, "mkfs.ext4", device);
    // ... actual work
}
```

## 10. UX Flow & Keybind Standardization

| Finding | Impact | Evidence | Recommendation |
|---------|--------|----------|----------------|
| **GOOD**: Bottom bar shows key legends | High | `[Space] Select  [Enter] Next  [Esc] Back  [q] Quit` | Keep as-is |
| **GOOD**: Three-pane layout | High | Left sidebar, center content, right system info | Keep as-is |
| **GAP**: `DESTROY` must be typed exactly | Medium | Case-sensitive, no feedback until complete | Show character-by-character feedback |
| **GAP**: No way to go back from Review | Low | User must quit and restart | Add "Back" option on Review screen |
| **GAP**: Disk selection shows `/dev/sda` not friendly name | Medium | Users don't know which disk is which | Show model + size prominently |

---

# D) Rust-First Replacement Plan (No External Commands)

## Current External Dependencies (14 Tools)

| Tool | Current Location | Calls/Install | Purpose |
|------|------------------|---------------|---------|
| `sync` | linux_hal.rs:319 | 1 | Flush buffers to disk |
| `udevadm` | linux_hal.rs:325 | 1 | Wait for kernel device events |
| `lsblk` | linux_hal.rs:334,350 | 2 | List block devices |
| `blkid` | linux_hal.rs:363 | 1 | Get filesystem UUID |
| `wipefs` | linux_hal.rs:377 | 1 | Clear filesystem signatures |
| `parted` | linux_hal.rs:395 | 4-6 | Partition table manipulation |
| `losetup` | linux_hal.rs:450,468 | 2 | Loop device setup/teardown |
| `mkfs.ext4` | linux_hal.rs:203 | 1 | Create ext4 filesystem |
| `mkfs.btrfs` | linux_hal.rs:214 | 1 | Create btrfs filesystem |
| `mkfs.vfat` | linux_hal.rs:238 | 1 | Create FAT32 filesystem |
| `btrfs` | linux_hal.rs:480,490 | 2 | Btrfs subvolume ops |
| `rsync` | linux_hal.rs:502 | 1 | Recursive copy with perms |
| `mount` | nix crate | - | (Already Rust via nix) |
| `umount` | nix crate | - | (Already Rust via nix) |

## Replacement Strategy

### Tier 1: Trivial Syscall Replacements

| Tool | Rust Replacement | Crate/Syscall | Risk | Effort |
|------|------------------|---------------|------|--------|
| `sync` | `libc::sync()` | libc (direct syscall) | None | 1 hour |
| `udevadm settle` | netlink socket monitor | `netlink-sys` / `tokio-udev` | Low | 1 day |

**Implementation for `sync`**:
```rust
// Replace Command::new("sync")
fn sync(&self) -> HalResult<()> {
    unsafe { libc::sync(); }
    Ok(())
}
```

### Tier 2: Sysfs/Procfs Parsing (Already Partially Done)

| Tool | Rust Replacement | Crate/Syscall | Risk | Effort |
|------|------------------|---------------|------|--------|
| `lsblk` | `/sys/block/` parsing | std::fs | Low | 2 days |
| `blkid` | `/dev/disk/by-uuid/` symlinks | std::fs::read_link | Low | 4 hours |

**Implementation for `lsblk`**:
```rust
// Already have procfs/sysfs modules — extend them
pub fn list_block_devices() -> HalResult<Vec<BlockDevice>> {
    let mut devices = Vec::new();
    for entry in std::fs::read_dir("/sys/block")? {
        let entry = entry?;
        let name = entry.file_name();

        // Skip loop/ram/dm devices
        let name_str = name.to_string_lossy();
        if name_str.starts_with("loop") || name_str.starts_with("ram") {
            continue;
        }

        // Read attributes from sysfs
        let size = read_sysfs_attr(&entry.path(), "size")?;
        let removable = read_sysfs_attr(&entry.path(), "removable")? == "1";
        let model = read_sysfs_attr(&entry.path().join("device"), "model").ok();

        devices.push(BlockDevice { name: name_str.into(), size, removable, model });
    }
    Ok(devices)
}
```

### Tier 3: Loop Device via ioctl

| Tool | Rust Replacement | Crate/Syscall | Risk | Effort |
|------|------------------|---------------|------|--------|
| `losetup` | `LOOP_CTL_GET_FREE` + `LOOP_SET_FD` | nix::ioctl | Medium | 2 days |

**Implementation**:
```rust
use nix::ioctl_none;
use nix::libc::{LOOP_CTL_GET_FREE, LOOP_SET_FD, LOOP_CLR_FD};
use std::os::unix::io::AsRawFd;

pub fn attach_loop(image: &Path) -> HalResult<PathBuf> {
    // 1. Open /dev/loop-control
    let ctl = File::open("/dev/loop-control")?;

    // 2. Get free loop number
    let num: i32 = unsafe { libc::ioctl(ctl.as_raw_fd(), LOOP_CTL_GET_FREE) };
    if num < 0 { return Err(HalError::Io(std::io::Error::last_os_error())); }

    // 3. Open loop device
    let loop_path = PathBuf::from(format!("/dev/loop{}", num));
    let loop_file = OpenOptions::new().read(true).write(true).open(&loop_path)?;

    // 4. Open image file
    let image_file = File::open(image)?;

    // 5. Attach
    let ret = unsafe { libc::ioctl(loop_file.as_raw_fd(), LOOP_SET_FD, image_file.as_raw_fd()) };
    if ret < 0 { return Err(HalError::Io(std::io::Error::last_os_error())); }

    Ok(loop_path)
}
```

### Tier 4: Partition Table Manipulation (HARD)

| Tool | Rust Replacement | Crate/Syscall | Risk | Effort |
|------|------------------|---------------|------|--------|
| `parted` | `gpt` crate + MBR writer | `gpt` (GPT), custom (MBR) | **High** | 2 weeks |
| `wipefs` | Direct sector zeroing | std::fs (write zeros) | Medium | 2 days |

**Honest Assessment**: Partition table manipulation is complex. The `gpt` crate handles GPT well but MBR requires custom code. Protective MBR for GPT, partition alignment, and edge cases make this non-trivial.

**Implementation for `wipefs` (simplified)**:
```rust
pub fn wipe_signatures(device: &Path, opts: &WipeFsOptions) -> HalResult<()> {
    destructive_guard!(opts, "wipefs", device);

    let mut file = OpenOptions::new().write(true).open(device)?;

    // Zero first 1MB (covers MBR, GPT headers, most FS superblocks)
    let zeros = vec![0u8; 1024 * 1024];
    file.write_all(&zeros)?;

    // Zero last 1MB (backup GPT)
    file.seek(SeekFrom::End(-(1024 * 1024)))?;
    file.write_all(&zeros)?;

    file.sync_all()?;
    Ok(())
}
```

**Implementation for GPT partitioning**:
```rust
use gpt::{GptConfig, partition_types};

pub fn create_gpt_table(device: &Path, partitions: &[PartitionSpec]) -> HalResult<()> {
    let cfg = GptConfig::new().writable(true);
    let mut disk = cfg.open(device)?;

    // Create new GPT
    disk.update_partitions(std::collections::BTreeMap::new())?;

    let mut start_lba = 2048; // Standard alignment
    for (i, spec) in partitions.iter().enumerate() {
        let size_sectors = spec.size_bytes / 512;
        disk.add_partition(
            &spec.name,
            size_sectors,
            partition_types::LINUX_FS,  // Adjust per spec.fs_type
            0,  // flags
            Some(start_lba),
        )?;
        start_lba += size_sectors;
    }

    disk.write()?;
    Ok(())
}
```

### Tier 5: Filesystem Creation (VERY HARD)

| Tool | Rust Replacement | Crate/Syscall | Risk | Effort |
|------|------------------|---------------|------|--------|
| `mkfs.vfat` | `fatfs` crate | `fatfs` | Medium | 1 week |
| `mkfs.ext4` | **None viable** | (see below) | **Very High** | 2+ months |
| `mkfs.btrfs` | **None viable** | (see below) | **Very High** | 3+ months |

**Honest Assessment**:
- **FAT32**: The `fatfs` crate can create FAT filesystems. Viable.
- **ext4**: No production-ready Rust crate. Would require implementing:
  - Superblock, block groups, inode tables
  - Journal (ext4 requires journaling by default)
  - Extent tree, directory hashing
  - This is essentially reimplementing `e2fsprogs` — **not realistic short-term**
- **btrfs**: Even more complex (copy-on-write, checksumming, multiple device support). **Not realistic**.

**Recommended Approach for ext4/btrfs**:
1. **Short-term**: Keep as external commands but document as "required dependencies"
2. **Medium-term**: Use FFI bindings to `libext2fs` (from e2fsprogs)
3. **Long-term**: Contribute to or adopt pure-Rust FS libraries when they mature

**FAT32 Implementation (viable)**:
```rust
use fatfs::{FatType, FormatVolumeOptions, format_volume};

pub fn format_fat32(device: &Path, label: &str, opts: &FormatOptions) -> HalResult<()> {
    destructive_guard!(opts, "mkfs.vfat", device);

    let file = OpenOptions::new().read(true).write(true).open(device)?;
    let options = FormatVolumeOptions::new()
        .fat_type(FatType::Fat32)
        .volume_label(label.as_bytes().try_into().unwrap_or(*b"NO NAME    "));

    format_volume(&file, options)?;
    Ok(())
}
```

### Tier 6: Recursive Copy with Permissions (HARD)

| Tool | Rust Replacement | Crate/Syscall | Risk | Effort |
|------|------------------|---------------|------|--------|
| `rsync` | Custom walker + syscalls | `walkdir` + nix | **High** | 3 weeks |

**Honest Assessment**: rsync does:
- Recursive directory traversal
- Permission/ownership preservation
- Extended attributes (xattr)
- Hard link detection
- Sparse file handling
- Delta transfer (when updating)
- Progress reporting

A minimal replacement for "copy from mounted image to target" is doable but won't have rsync's robustness.

**Minimal Implementation**:
```rust
use walkdir::WalkDir;
use nix::sys::stat::{fchmodat, FchmodatFlags};
use nix::unistd::{chown, Uid, Gid};

pub fn copy_tree(
    src: &Path,
    dst: &Path,
    on_progress: &mut dyn FnMut(u64, u64) -> bool
) -> HalResult<()> {
    let mut total_bytes = 0u64;
    let mut copied_bytes = 0u64;

    // First pass: calculate total size
    for entry in WalkDir::new(src) {
        let entry = entry?;
        if entry.file_type().is_file() {
            total_bytes += entry.metadata()?.len();
        }
    }

    // Second pass: copy
    for entry in WalkDir::new(src) {
        let entry = entry?;
        let rel = entry.path().strip_prefix(src)?;
        let dst_path = dst.join(rel);

        let meta = entry.metadata()?;

        if entry.file_type().is_dir() {
            std::fs::create_dir_all(&dst_path)?;
        } else if entry.file_type().is_symlink() {
            let target = std::fs::read_link(entry.path())?;
            std::os::unix::fs::symlink(&target, &dst_path)?;
        } else if entry.file_type().is_file() {
            std::fs::copy(entry.path(), &dst_path)?;
            copied_bytes += meta.len();

            if !on_progress(copied_bytes, total_bytes) {
                return Err(HalError::Other("Cancelled".into()));
            }
        }

        // Preserve ownership and permissions (requires CAP_CHOWN)
        let uid = Uid::from_raw(meta.uid());
        let gid = Gid::from_raw(meta.gid());
        let _ = chown(&dst_path, Some(uid), Some(gid));
        let _ = std::fs::set_permissions(&dst_path, meta.permissions());

        // Preserve xattrs (requires xattr crate)
        #[cfg(feature = "xattr")]
        for name in xattr::list(entry.path())? {
            if let Ok(value) = xattr::get(entry.path(), &name) {
                let _ = xattr::set(&dst_path, &name, &value.unwrap_or_default());
            }
        }
    }

    Ok(())
}
```

### Tier 7: Btrfs Subvolumes (MEDIUM)

| Tool | Rust Replacement | Crate/Syscall | Risk | Effort |
|------|------------------|---------------|------|--------|
| `btrfs subvol create` | `BTRFS_IOC_SUBVOL_CREATE` | nix::ioctl | Medium | 3 days |
| `btrfs subvol list` | `BTRFS_IOC_TREE_SEARCH` | nix::ioctl | Medium | 3 days |

**Implementation**:
```rust
use nix::libc;
use std::os::unix::io::AsRawFd;

const BTRFS_IOC_SUBVOL_CREATE: u64 = 0x5014_940e; // From btrfs headers

#[repr(C)]
struct BtrfsIoctlVolArgs {
    fd: i64,
    name: [u8; 4040],
}

pub fn create_subvolume(parent: &Path, name: &str) -> HalResult<()> {
    let dir = File::open(parent)?;

    let mut args = BtrfsIoctlVolArgs {
        fd: 0,
        name: [0u8; 4040],
    };

    let name_bytes = name.as_bytes();
    args.name[..name_bytes.len()].copy_from_slice(name_bytes);

    let ret = unsafe {
        libc::ioctl(dir.as_raw_fd(), BTRFS_IOC_SUBVOL_CREATE, &args)
    };

    if ret < 0 {
        return Err(HalError::Io(std::io::Error::last_os_error()));
    }

    Ok(())
}
```

## Summary Table

| Tool | Rust Viable? | Recommended Approach | Effort |
|------|--------------|----------------------|--------|
| sync | Yes | `libc::sync()` | Trivial |
| udevadm | Yes | netlink socket | 1 day |
| lsblk | Yes | sysfs parsing | 2 days |
| blkid | Yes | `/dev/disk/by-uuid/` | 4 hours |
| losetup | Yes | Loop ioctls | 2 days |
| wipefs | Yes | Direct sector zeroing | 2 days |
| parted | Partial | `gpt` crate (GPT only); MBR needs custom | 2 weeks |
| mkfs.vfat | Yes | `fatfs` crate | 1 week |
| mkfs.ext4 | **No** | Keep command OR FFI to libext2fs | N/A |
| mkfs.btrfs | **No** | Keep command | N/A |
| btrfs subvol | Yes | ioctl direct | 3 days |
| rsync | Partial | Custom walker; less robust | 3 weeks |

---

# E) Proposed Work Orders / Phases

## Phase 1: Foundation Hardening (Release Blocker)

**Scope**:
- Unify error types (`HalError` + `MashError` → single hierarchy)
- Add idempotency guards to mount/unmount
- Replace `sync` command with syscall
- Add RAII guards for mounts/loops

**Non-Goals**:
- No external command replacement (except sync)
- No TUI changes

**Acceptance Criteria**:
- [ ] Single `MashError` enum with `Hal` variant
- [ ] `mount_device()` is idempotent (no error if already mounted)
- [ ] `unmount_recursive()` is idempotent
- [ ] `MountGuard` and `LoopGuard` types exist and auto-cleanup on drop
- [ ] `cargo test` passes
- [ ] CI passes with `--all-features`

**Tests Required**:
- Unit: Mount idempotency with FakeHal
- Unit: Guard cleanup on panic (use `catch_unwind`)
- Integration: Full pipeline dry-run

## Phase 2: Sysfs Native Probing

**Scope**:
- Replace `lsblk` with `/sys/block/` parsing
- Replace `blkid` with `/dev/disk/by-uuid/` parsing
- Replace `udevadm settle` with netlink monitoring

**Non-Goals**:
- No partitioning changes
- No format changes

**Acceptance Criteria**:
- [ ] `ProbeOps` no longer calls `lsblk` or `blkid`
- [ ] `SystemOps::udev_settle()` uses netlink
- [ ] Disk listing includes model/serial from sysfs
- [ ] TUI disk selection shows stable identifiers

**Tests Required**:
- Unit: sysfs parser with sample `/sys/block` tree
- Unit: Netlink event simulation
- Integration: Disk detection on real hardware

## Phase 3: Loop and Wipe Native

**Scope**:
- Replace `losetup` with loop ioctls
- Replace `wipefs` with direct sector zeroing
- Add partition scan trigger via `BLKRRPART` ioctl

**Non-Goals**:
- No partition table creation
- No filesystem creation

**Acceptance Criteria**:
- [ ] `LoopOps` uses ioctls, not commands
- [ ] `wipefs_all()` zeros sectors directly
- [ ] Partition re-read uses `BLKRRPART` ioctl

**Tests Required**:
- Unit: Loop attach/detach with temp files
- Unit: Wipefs with small test file
- Integration: Loop mount of real .img file

## Phase 4: Partition Table Native (GPT-first)

**Scope**:
- Implement GPT partitioning via `gpt` crate
- Implement basic MBR creation (type 0x83 Linux)
- Keep `parted` as fallback for edge cases

**Non-Goals**:
- Full MBR feature parity
- LVM/RAID support

**Acceptance Criteria**:
- [ ] GPT table creation works without `parted`
- [ ] Basic MBR (EFI + root) works without `parted`
- [ ] Fallback to `parted` logged and documented
- [ ] Partition alignment at 1MiB boundaries

**Tests Required**:
- Unit: GPT creation with `gpt` crate on temp file
- Unit: MBR creation on temp file
- Integration: Full partition cycle on loop device

## Phase 5: FAT32 Native

**Scope**:
- Replace `mkfs.vfat` with `fatfs` crate
- Verify EFI partition is bootable

**Non-Goals**:
- ext4/btrfs native (kept as commands)

**Acceptance Criteria**:
- [ ] EFI partition formatted by `fatfs`
- [ ] Bootloader files writable to formatted partition
- [ ] UEFI firmware can read partition

**Tests Required**:
- Unit: FAT32 format and file write
- Integration: Boot test on real Pi hardware

## Phase 6: Copy Tree Native

**Scope**:
- Implement `copy_tree()` function with:
  - Permission preservation
  - Symlink handling
  - Progress callbacks
  - xattr support (optional feature)
- Keep `rsync` as opt-in fallback

**Non-Goals**:
- Delta transfer
- Hard link deduplication

**Acceptance Criteria**:
- [ ] `RsyncOps` can use native implementation
- [ ] Progress reporting works
- [ ] Permissions match source
- [ ] Symlinks preserved
- [ ] `--rsync-fallback` flag available

**Tests Required**:
- Unit: Copy with various file types
- Unit: Permission preservation
- Integration: Full OS copy comparison (native vs rsync)

## Phase 7: TUI Refactor (Post-Release)

**Scope**:
- Split `dojo_app.rs` (2,652 LOC) into:
  - `dojo_state.rs` — State machine
  - `dojo_render.rs` — UI rendering
  - `dojo_input.rs` — Key handling
  - `dojo_data.rs` — Data fetching
- Add TUI state unit tests

**Non-Goals**:
- Feature additions
- New screens

**Acceptance Criteria**:
- [ ] No file > 500 LOC
- [ ] State transitions have unit tests
- [ ] Rendering is pure (no side effects)

**Tests Required**:
- Unit: State machine transitions
- Unit: Key input mapping
- Visual: Screenshot comparison tests

## No-Regression Strategy

1. **Feature flags**: New implementations behind `--native-*` flags
2. **A/B comparison**: Run both paths in CI, compare results
3. **FakeHal parity**: Every native impl mirrored in FakeHal
4. **Dry-run first**: All changes tested in dry-run before armed mode
5. **Hardware matrix**: Test on Pi 4B 1GB, 4GB, and 8GB variants
6. **Rollback path**: Keep command-based impl available via feature flag

---

# F) Concrete Examples

## Example 1: Duplicated Function → Consolidated

**Location**: `linux_hal.rs:191-198` (repeated 6 times)

```rust
// BEFORE: Duplicated in format_ext4, format_btrfs, format_vfat, wipefs_all, parted, flash_raw_image
if opts.dry_run {
    log::info!("DRY RUN: mkfs.ext4 {}", device.display());
    return Ok(());
}
if !opts.confirmed {
    return Err(HalError::SafetyLock);
}

// AFTER: mash-hal/src/macros.rs
#[macro_export]
macro_rules! destructive_op {
    ($opts:expr, $op:literal, $target:expr) => {
        if $opts.dry_run {
            log::info!(concat!("DRY RUN: ", $op, " {}"), $target.display());
            return Ok(());
        }
        if !$opts.confirmed {
            return Err($crate::HalError::SafetyLock);
        }
    };
}

// Usage
fn format_ext4(&self, device: &Path, opts: &FormatOptions) -> HalResult<()> {
    destructive_op!(opts, "mkfs.ext4", device);
    // ... actual formatting
}
```

**Lines saved**: ~60 LOC

## Example 2: String Error → Typed Error

**Location**: `downloader/mod.rs:165-170`

```rust
// BEFORE
if computed != self.checksum {
    anyhow::bail!("checksum mismatch: {} != {}", computed, self.checksum);
}

// AFTER (in mash-core/src/errors.rs)
#[derive(Error, Debug)]
pub enum DownloadError {
    #[error("Checksum mismatch for {file}: computed {computed}, expected {expected}")]
    ChecksumMismatch {
        file: PathBuf,
        computed: String,
        expected: String,
    },

    #[error("Download failed after {attempts} retries: {reason}")]
    RetryExhausted {
        attempts: usize,
        reason: String,
    },
}

// Usage
if computed != self.checksum {
    return Err(DownloadError::ChecksumMismatch {
        file: self.path.clone(),
        computed,
        expected: self.checksum.clone(),
    }.into());
}
```

**Benefit**: Pattern matching, structured logging, better error messages

## Example 3: Missing Idempotency Guard

**Location**: `linux_hal.rs:116-139`

```rust
// BEFORE: Fails with EBUSY if already mounted
fn mount_device(&self, device: &Path, target: &Path, fstype: Option<&str>, options: MountOptions, dry_run: bool) -> HalResult<()> {
    if dry_run {
        log::info!("DRY RUN: mount {} -> {}", device.display(), target.display());
        return Ok(());
    }
    nix::mount::mount(Some(device), target, fstype, flags, data)
        .map_err(map_nix_err)?;  // EBUSY if already mounted!
    Ok(())
}

// AFTER: Idempotent
fn mount_device(&self, device: &Path, target: &Path, fstype: Option<&str>, options: MountOptions, dry_run: bool) -> HalResult<()> {
    if dry_run {
        log::info!("DRY RUN: mount {} -> {}", device.display(), target.display());
        return Ok(());
    }

    // Idempotency guard
    if self.is_mounted(target)? {
        log::debug!("Already mounted: {} -> {}", device.display(), target.display());
        return Ok(());
    }

    // Create mount point if needed
    if !target.exists() {
        std::fs::create_dir_all(target)?;
    }

    nix::mount::mount(Some(device), target, fstype, flags, data)
        .map_err(map_nix_err)?;
    Ok(())
}
```

**Benefit**: Safe to call multiple times, crash recovery works

## Example 4: Performance Hotspot

**Location**: `linux_hal.rs:548`

```rust
// BEFORE: Unbounded channel can grow without limit on slow TUI
let (tx, rx) = mpsc::channel::<io::Result<String>>();

// Measurement approach:
// 1. Add counter: static CHANNEL_DEPTH: AtomicUsize
// 2. Increment on send, decrement on receive
// 3. Log max depth every 1000 messages
// 4. Profile with `perf record` during rsync of large filesystem

// AFTER: Bounded channel with backpressure
let (tx, rx) = mpsc::sync_channel::<io::Result<String>>(64);

// If sender blocks, rsync naturally slows down (which is fine)
// Memory usage capped at 64 * ~100 bytes = ~6KB
```

**Measurement**: Use `heaptrack` to profile memory during 10GB rsync

## Example 5: UX Confusion Point

**Location**: `dojo_app.rs` — DESTROY confirmation flow

```
// CURRENT: User types "DESTROY", sees nothing until complete
┌─────────────────────────────────────────┐
│  Type DESTROY to arm destructive mode:  │
│  > ______                               │
│                                         │
│  (no feedback as user types)            │
└─────────────────────────────────────────┘

// IMPROVED: Character-by-character feedback
┌─────────────────────────────────────────┐
│  Type DESTROY to arm destructive mode:  │
│  > DEST___                              │
│    ████░░░  (4/7 characters)            │
│                                         │
│  Press Esc to cancel                    │
└─────────────────────────────────────────┘

// Implementation in dojo_app.rs
fn render_arming_prompt(&self, frame: &mut Frame, area: Rect) {
    let typed = &self.arming_buffer;
    let target = "DESTROY";
    let progress = typed.len() as f32 / target.len() as f32;

    // Show masked input with progress
    let display: String = target.chars()
        .enumerate()
        .map(|(i, c)| if i < typed.len() { c } else { '_' })
        .collect();

    let gauge = Gauge::default()
        .ratio(progress)
        .label(format!("{}/{}", typed.len(), target.len()));

    // Render both
}
```

**Benefit**: User knows they're making progress, reduces re-typing frustration

---

# G) Risk Register

## Top 10 Risks of "No External Commands"

| # | Risk | Likelihood | Impact | Mitigation |
|---|------|------------|--------|------------|
| 1 | **ext4/btrfs creation impossible in pure Rust** | Certain | High | Keep `mkfs.*` as documented dependency; pursue FFI bindings to libext2fs |
| 2 | **Custom GPT writer has edge cases** | High | High | Extensive testing; keep `parted` fallback; fuzzing with disk images |
| 3 | **Copy tree misses permissions/xattrs** | Medium | High | Test against rsync output; add `--verify` mode that compares |
| 4 | **Loop ioctl varies across kernel versions** | Low | Medium | Test on 4.x, 5.x, 6.x kernels; graceful degradation |
| 5 | **Netlink udev monitoring races** | Medium | Medium | Add timeout + retry; keep `udevadm` fallback for diagnostics |
| 6 | **sysfs parsing breaks on unusual devices** | Low | Low | Validate against known Pi hardware; fallback to lsblk |
| 7 | **FAT32 implementation rejected by UEFI** | Low | High | Test on multiple UEFI implementations; validate with real Pi boot |
| 8 | **Memory usage higher than command approach** | Medium | Medium | Profile on 1GB Pi; add `--low-memory` mode |
| 9 | **MBR implementation incomplete** | Medium | Medium | Support only EFI+root layout; document limitations |
| 10 | **Btrfs ioctl interface undocumented** | Low | Low | Use kernel headers; test specific kernel versions |

## Risk Mitigation Without Shell Commands

| Risk | Mitigation Strategy |
|------|---------------------|
| mkfs impossible | Phase approach: FAT32 native, ext4/btrfs via FFI, long-term pure Rust |
| Partition edge cases | Comprehensive test suite with real disk images; `gpt` crate is well-maintained |
| Copy completeness | Verification pass comparing source/dest metadata |
| Kernel compatibility | CI matrix with multiple kernel versions |
| Boot failure | Test matrix with real hardware before release |

## Fallback Strategy (NO Shell Commands)

If a pure-Rust implementation fails at runtime:

1. **Log detailed error** with context
2. **Suggest manual steps** the user can take
3. **Save state** for resume after manual intervention
4. **Do NOT silently shell out** — that defeats the purpose

Example:
```
ERROR: Native ext4 formatting failed: Unsupported feature X
       This may require manual formatting.

MANUAL STEPS:
  1. Run: sudo mkfs.ext4 -L ROOT /dev/sda2
  2. Re-run MASH with --resume flag

State saved to ~/.mash/state.json
```

---

# Multi-Hat Findings Summary

## Larry Hat (Engineer / Refactor)
- **Action**: Split `dojo_app.rs` (2,652 LOC) into 4 modules
- **Action**: Consolidate timeout helpers into shared crate
- **Action**: Create `destructive_op!` macro for DRY
- **Action**: Profile rsync memory on 1GB Pi

## Moe Hat (Architect / QA)
- **Release Blocker**: Idempotency guards for mount/unmount
- **Release Blocker**: RAII cleanup guards
- **Post-Release**: Native copy tree implementation
- **Post-Release**: Native partitioning

## Curly Hat (Pragmatic Glue)
- **Quick Win**: Replace `sync` with syscall (1 hour)
- **Quick Win**: Add `is_mounted()` check (2 hours)
- **Quick Win**: Bounded channel for rsync progress (1 hour)
- **Document**: Required external tools (mkfs.ext4, mkfs.btrfs)

## Claude Hat (UX/TUI)
- **Improve**: DESTROY typing feedback (character-by-character)
- **Improve**: Disk selection shows model + size prominently
- **Add**: "Back" option on Review screen
- **Add**: `--log-file` option for debugging

## Idiot User Hat (Hostile Reality)
- **Problem**: User has one SD card, one Pi, phone hotspot only
- **Problem**: Power loss mid-install = bricked card
- **Problem**: "Which disk is /dev/sda?" — they don't know
- **Solution**: Show "SanDisk Ultra 32GB (USB)" not "/dev/sda"
- **Solution**: Warn if target is only bootable media
- **Solution**: Checkpoint every major stage for resume

---

# Conclusion

MASH is well-architected with strong safety foundations. The path to "Rust all the way down" is achievable for ~80% of operations, with honest acknowledgment that **ext4 and btrfs filesystem creation will remain as external dependencies** until viable Rust crates exist.

The recommended approach:
1. **Immediate**: Hardening (idempotency, RAII, error unification)
2. **Short-term**: Native sysfs/loop/wipe (eliminate 6 commands)
3. **Medium-term**: Native GPT + FAT32 (eliminate 2 more)
4. **Long-term**: Native copy tree, FFI for ext4

Total external command reduction: **14 → 3** (mkfs.ext4, mkfs.btrfs, rsync-fallback)

This maintains the "production-grade one-shot installer" goal while being honest about Rust ecosystem limitations.

---

*Review completed by Claude (French), Engineering Reviewer*
*Report generated: 2026-02-04*
