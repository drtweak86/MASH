# MASH Refactoring Plan

**Goal**: Deduplicated, idempotent, hardened, efficient codebase relying solely on Rust and Rust tools.

**Scope**: All code excluding `legacy/` and `legacy_scripts/` directories.

**Codebase Stats**:
- ~7,054 LOC across 5 main crates
- 15 external shell commands currently in use
- 154 unwrap/expect instances requiring review
- 45+ dry-run check duplications

---

## Phase 1: Foundation Hardening (Priority: Critical)

### 1.1 Consolidate Error Types

**Problem**: `MashError` (mash-core) and `HalError` (mash-hal) have overlapping variants (SafetyLock, DiskBusy, PermissionDenied).

**Solution**: Create unified error hierarchy in `mash-core`:

```
mash-core/src/error.rs
├── MashError (top-level)
│   ├── Hal(HalError)      # Wraps HAL errors
│   ├── Download(...)
│   ├── Config(...)
│   └── Pipeline(...)
```

**Files to modify**:
- `mash-core/src/error.rs` - Define hierarchy
- `crates/mash-hal/src/error.rs` - Keep HAL-specific errors
- All callsites using `.map_err()` conversions

**Effort**: Medium | **Impact**: High (cleaner error propagation)

---

### 1.2 Eliminate Unwrap/Expect Panic Points

**Problem**: 154 instances of `unwrap()`/`expect()` can panic in privileged context.

**Strategy**:

| Pattern | Replacement |
|---------|-------------|
| `option.unwrap()` | `option.ok_or(MashError::Missing("context"))?` |
| `result.expect("msg")` | `result.context("msg")?` |
| `vec.get(i).unwrap()` | `vec.get(i).ok_or(MashError::IndexOutOfBounds)?` |

**Priority files** (by unwrap count):
1. `mash-core/src/flash.rs` (~40 instances)
2. `mash-core/src/downloader/mod.rs` (~25 instances)
3. `crates/mash-tui/src/dojo/dojo_app.rs` (~20 instances)
4. `crates/mash-hal/src/hal/linux_hal.rs` (~15 instances)

**Effort**: High | **Impact**: Critical (prevents production panics)

---

### 1.3 Unify Nix Crate Version

**Problem**: Version mismatch between crates:
- `mash-hal`: nix 0.29
- `mash-core`: nix 0.27

**Solution**: Upgrade all to nix 0.29 in workspace `Cargo.toml`:

```toml
[workspace.dependencies]
nix = { version = "0.29", features = ["mount", "fs"] }
```

**Effort**: Low | **Impact**: Medium (consistency, reduced binary size)

---

## Phase 2: Code Deduplication (Priority: High)

### 2.1 Consolidate Timeout Helpers

**Problem**: Duplicate timeout implementations in:
- `mash-core/src/process_timeout.rs` (76 LOC)
- `crates/mash-hal/src/hal/linux_hal.rs` (96 LOC inline)

**Solution**: Create shared utility crate or module:

```rust
// mash-core/src/util/command.rs
pub struct TimedCommand {
    cmd: Command,
    timeout: Duration,
    context: &'static str,
}

impl TimedCommand {
    pub fn new(program: &str) -> Self;
    pub fn args<I, S>(&mut self, args: I) -> &mut Self;
    pub fn timeout(&mut self, d: Duration) -> &mut Self;
    pub fn run(&mut self) -> Result<Output, MashError>;
    pub fn run_streaming<F>(&mut self, on_line: F) -> Result<(), MashError>;
}
```

**Files to modify**:
- Create `mash-core/src/util/command.rs`
- Update `linux_hal.rs` to use shared utility
- Remove `process_timeout.rs` after migration

**Effort**: Medium | **Impact**: High (removes ~170 LOC duplication)

---

### 2.2 Extract Dry-Run Pattern to Macro

**Problem**: 45+ instances of:
```rust
if opts.dry_run {
    log::info!("[DRY-RUN] Would do X");
    return Ok(());
}
```

**Solution**: Create macro in mash-hal:

```rust
// mash-hal/src/macros.rs
macro_rules! dry_run_guard {
    ($opts:expr, $action:expr) => {
        if $opts.dry_run {
            log::info!("[DRY-RUN] {}", $action);
            return Ok(());
        }
    };
}
```

**Usage**:
```rust
dry_run_guard!(opts, format!("Would format {} as ext4", device));
```

**Effort**: Low | **Impact**: Medium (cleaner code, consistent logging)

---

### 2.3 Extract Safety Check Pattern

**Problem**: 8+ instances of:
```rust
if !opts.confirmed {
    return Err(HalError::SafetyLock);
}
```

**Solution**: Add to `HalOpts` or trait:

```rust
impl HalOpts {
    pub fn require_armed(&self) -> Result<(), HalError> {
        if !self.confirmed {
            Err(HalError::SafetyLock)
        } else {
            Ok(())
        }
    }
}
```

**Usage**:
```rust
opts.require_armed()?;
```

**Effort**: Low | **Impact**: Medium

---

### 2.4 Consolidate Mount Path Builder

**Problem**: Manual path construction scattered across:
- `mash-core/src/flash.rs` (12+ instances)
- `crates/mash-tui/src/dojo/dojo_app.rs`

**Solution**: Create `MountLayout` struct:

```rust
// mash-core/src/mount_layout.rs
pub struct MountLayout {
    pub base: PathBuf,
}

impl MountLayout {
    pub fn new(base: impl AsRef<Path>) -> Self;
    pub fn root(&self) -> PathBuf;       // base/
    pub fn boot(&self) -> PathBuf;       // base/boot
    pub fn efi(&self) -> PathBuf;        // base/boot/efi
    pub fn etc(&self) -> PathBuf;        // base/etc
    pub fn fstab(&self) -> PathBuf;      // base/etc/fstab
    pub fn locale_conf(&self) -> PathBuf; // base/etc/locale.conf
}
```

**Effort**: Low | **Impact**: Medium (type-safe paths)

---

### 2.5 Consolidate Download Callback Pattern

**Problem**: Repeated closure pattern for cancellable downloads:
```rust
let mut cb = |p: DownloadProgress| {
    if cancel_flag.map(|flag| flag.load(Ordering::Relaxed)).unwrap_or(false) {
        return false;
    }
    progress(p)
};
```

**Solution**: Helper function:

```rust
// mash-core/src/downloader/callback.rs
pub fn cancellable_progress<F>(
    cancel: Option<&AtomicBool>,
    mut on_progress: F,
) -> impl FnMut(DownloadProgress) -> bool
where
    F: FnMut(DownloadProgress) -> bool,
{
    move |p| {
        if cancel.map(|f| f.load(Ordering::Relaxed)).unwrap_or(false) {
            return false;
        }
        on_progress(p)
    }
}
```

**Effort**: Low | **Impact**: Low

---

## Phase 3: Replace Shell Commands with Rust (Priority: High)

### 3.1 Replace `sync` with libc syscall

**Current** (`linux_hal.rs:~319`):
```rust
Command::new("sync").status()?;
```

**Replace with**:
```rust
// Direct syscall - no fork
unsafe { libc::sync(); }
```

**Effort**: Trivial | **Impact**: Medium (eliminates 1 fork per install)

---

### 3.2 Replace `lsblk` with /sys/block parsing

**Current**: Parses JSON output from `lsblk --json`

**Replace with**: Expand existing `/sys/block` infrastructure:

```rust
// mash-hal/src/sysfs/block.rs (extend existing)
pub fn list_block_devices() -> Result<Vec<BlockDevice>, HalError> {
    let mut devices = Vec::new();
    for entry in std::fs::read_dir("/sys/block")? {
        let entry = entry?;
        let name = entry.file_name();
        // Parse: size, removable, ro, model, serial from sysfs
        devices.push(BlockDevice::from_sysfs(&entry.path())?);
    }
    Ok(devices)
}
```

**Files**:
- Extend `crates/mash-hal/src/sysfs/block.rs`
- Update `probe_ops.rs` to use sysfs instead of lsblk

**Effort**: Medium | **Impact**: High (eliminates 3+ forks per probe)

---

### 3.3 Replace `blkid` with /dev parsing or libblkid

**Option A**: Parse `/dev/disk/by-uuid/` symlinks:
```rust
pub fn get_uuid(device: &Path) -> Result<String, HalError> {
    for entry in std::fs::read_dir("/dev/disk/by-uuid")? {
        let entry = entry?;
        let target = std::fs::read_link(entry.path())?;
        if target.file_name() == device.file_name() {
            return Ok(entry.file_name().to_string_lossy().to_string());
        }
    }
    Err(HalError::NotFound)
}
```

**Option B**: Use `blkid-sys` crate (FFI to libblkid)

**Recommendation**: Option A for simplicity, no new dependencies.

**Effort**: Low | **Impact**: Medium

---

### 3.4 Replace `losetup` with ioctl

**Current**: Forks to `losetup -f --show`

**Replace with**:
```rust
use nix::ioctl_write_ptr;
use std::os::unix::io::AsRawFd;

const LOOP_CTL_GET_FREE: u64 = 0x4C82;
const LOOP_SET_FD: u64 = 0x4C00;

pub fn attach_loop(image: &Path) -> Result<PathBuf, HalError> {
    // 1. Open /dev/loop-control
    let ctl = File::open("/dev/loop-control")?;

    // 2. Get free loop device number
    let num: i32 = unsafe {
        libc::ioctl(ctl.as_raw_fd(), LOOP_CTL_GET_FREE)
    };

    // 3. Open loop device and attach
    let loop_dev = PathBuf::from(format!("/dev/loop{}", num));
    let loop_file = OpenOptions::new().read(true).write(true).open(&loop_dev)?;
    let image_file = File::open(image)?;

    unsafe {
        libc::ioctl(loop_file.as_raw_fd(), LOOP_SET_FD, image_file.as_raw_fd());
    }

    Ok(loop_dev)
}
```

**Effort**: Medium | **Impact**: Medium (eliminates fork, better error handling)

---

### 3.5 Replace `udevadm settle` with netlink

**Current**: Blocking `udevadm settle` call

**Replace with**: Use `netlink` crate to monitor udev events:

```rust
// mash-hal/src/udev/mod.rs
use netlink_sys::{Socket, protocols::NETLINK_KOBJECT_UEVENT};

pub fn wait_for_device(device: &Path, timeout: Duration) -> Result<(), HalError> {
    let socket = Socket::new(NETLINK_KOBJECT_UEVENT)?;
    socket.bind(&SocketAddr::new(0, 1))?; // Listen for kernel events

    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        // Poll for events with remaining timeout
        // Check if target device appears
    }
    Err(HalError::Timeout)
}
```

**Effort**: High | **Impact**: Medium (proper cancellation, no blocking fork)

---

### 3.6 Shell Commands to Keep (Not Replaceable)

| Command | Reason |
|---------|--------|
| `parted` | GPT/MBR manipulation - no stable Rust alternative |
| `mkfs.ext4/btrfs/vfat` | Filesystem creation - requires kernel features |
| `rsync` | Complex delta-sync algorithm - no Rust equivalent |
| `btrfs` | Subvolume management - kernel ioctl complexity |
| `wipefs` | Keep for now, could replace with direct zeroing |
| `dnf`/`systemctl` | System tools - must use native |

**Strategy**: Wrap these in the `TimedCommand` abstraction from 2.1.

---

## Phase 4: Idempotency Improvements (Priority: Medium)

### 4.1 Add Operation Guards

**Problem**: Some operations fail if run twice (e.g., mount already mounted).

**Solution**: Pre-check state before operations:

```rust
// mash-hal/src/mount_ops.rs
impl MountOps for LinuxHal {
    fn mount(&self, source: &Path, target: &Path, ...) -> Result<(), HalError> {
        // Guard: already mounted?
        if self.is_mounted(target)? {
            log::debug!("{} already mounted, skipping", target.display());
            return Ok(());
        }
        // Proceed with mount
    }
}
```

**Apply to**:
- `mount()` - check `/proc/self/mountinfo`
- `format()` - check filesystem type matches expected
- `partition()` - check partition table exists and matches

**Effort**: Medium | **Impact**: High (safe re-runs)

---

### 4.2 Checkpoint Validation

**Current**: Checkpoint system exists in `state_manager/`

**Enhancement**: Add integrity checks:

```rust
// mash-core/src/state_manager/mod.rs
impl StateManager {
    pub fn validate_checkpoint(&self) -> Result<CheckpointValidity, MashError> {
        // Verify:
        // 1. Target disk still exists
        // 2. Partitions match expected layout
        // 3. No conflicting mounts
        // 4. Downloaded image checksum still valid
    }
}
```

**Effort**: Medium | **Impact**: High (safe resume)

---

### 4.3 Transactional Cleanup

**Problem**: Error paths may leave resources (mounts, loop devices) attached.

**Solution**: RAII guards for resources:

```rust
// mash-hal/src/guards.rs
pub struct MountGuard<'a> {
    hal: &'a dyn Hal,
    target: PathBuf,
}

impl Drop for MountGuard<'_> {
    fn drop(&mut self) {
        if let Err(e) = self.hal.unmount(&self.target) {
            log::warn!("Failed to unmount {}: {}", self.target.display(), e);
        }
    }
}

pub struct LoopGuard<'a> {
    hal: &'a dyn Hal,
    device: PathBuf,
}

impl Drop for LoopGuard<'_> {
    fn drop(&mut self) {
        if let Err(e) = self.hal.detach_loop(&self.device) {
            log::warn!("Failed to detach {}: {}", self.device.display(), e);
        }
    }
}
```

**Effort**: Medium | **Impact**: High (resource leak prevention)

---

## Phase 5: Efficiency Optimizations (Priority: Low)

### 5.1 Parallel Preflight Checks

**Current**: Sequential checks in `preflight.rs`

**Optimize**: Run independent checks in parallel:

```rust
use rayon::prelude::*;

pub fn run_preflight(hal: &dyn Hal) -> Result<PreflightReport, MashError> {
    let checks: Vec<Box<dyn Fn() -> CheckResult + Send + Sync>> = vec![
        Box::new(|| check_root_privileges()),
        Box::new(|| check_disk_space(hal)),
        Box::new(|| check_network_connectivity()),
        Box::new(|| check_required_tools()),
    ];

    let results: Vec<_> = checks.par_iter()
        .map(|check| check())
        .collect();

    // Aggregate results
}
```

**Effort**: Low | **Impact**: Low (faster startup)

---

### 5.2 Streaming Checksum Verification

**Current**: Download complete, then verify checksum.

**Optimize**: Calculate checksum while streaming:

```rust
// Already partially implemented in downloader
// Ensure all paths use streaming verification
```

**Status**: Verify current implementation

**Effort**: Low | **Impact**: Medium (no second pass over data)

---

### 5.3 Reduce Allocations in Progress Callbacks

**Current**: Progress updates may allocate strings repeatedly.

**Optimize**: Use `Cow<'static, str>` for phase names:

```rust
pub struct ProgressUpdate {
    pub phase: Cow<'static, str>,  // Avoids allocation for known phases
    pub percent: u8,
    pub bytes_done: u64,
}
```

**Effort**: Low | **Impact**: Low

---

## Implementation Order

### Sprint 1: Foundation (1-2 weeks)
1. [ ] 1.3 - Unify nix version
2. [ ] 2.2 - Dry-run macro
3. [ ] 2.3 - Safety check helper
4. [ ] 3.1 - Replace sync with libc

### Sprint 2: Deduplication (2-3 weeks)
1. [ ] 2.1 - Consolidate timeout helpers
2. [ ] 2.4 - Mount path builder
3. [ ] 1.1 - Error type consolidation

### Sprint 3: Shell Replacement (2-3 weeks)
1. [ ] 3.2 - Replace lsblk with sysfs
2. [ ] 3.3 - Replace blkid with /dev/disk
3. [ ] 3.4 - Replace losetup with ioctl

### Sprint 4: Hardening (2-3 weeks)
1. [ ] 1.2 - Eliminate unwrap/expect (systematic review)
2. [ ] 4.3 - RAII guards for resources
3. [ ] 4.1 - Idempotent operation guards

### Sprint 5: Polish (1-2 weeks)
1. [ ] 4.2 - Checkpoint validation
2. [ ] 3.5 - Netlink udev (optional)
3. [ ] 5.1-5.3 - Efficiency optimizations

---

## Success Metrics

| Metric | Before | Target |
|--------|--------|--------|
| Shell command forks | 15 | 8 |
| unwrap/expect instances | 154 | <20 |
| Duplicate code patterns | 5 major | 0 |
| Test coverage | TBD | >80% |
| CI lint warnings | 0 | 0 |

---

## Files Changed Summary

| File | Changes |
|------|---------|
| `mash-core/src/error.rs` | New unified error hierarchy |
| `mash-core/src/util/command.rs` | New TimedCommand utility |
| `mash-core/src/mount_layout.rs` | New path builder |
| `mash-hal/src/macros.rs` | New dry-run macro |
| `mash-hal/src/sysfs/block.rs` | Extended for lsblk replacement |
| `mash-hal/src/hal/linux_hal.rs` | Major refactor |
| `mash-hal/src/guards.rs` | New RAII guards |
| `mash-core/src/flash.rs` | Use new utilities |
| `Cargo.toml` (workspace) | Dependency alignment |

---

## Risk Assessment

| Risk | Mitigation |
|------|------------|
| Breaking HAL interface | Keep FakeHal in sync, run full test suite |
| Regression in install pipeline | Maintain dry-run coverage, test on real hardware periodically |
| ioctl complexity | Start with simple cases, keep command fallback |
| CI breakage | Run `make lint && make test` before each PR |

---

## References

- Current codebase exploration: Agent abc8ab7
- CLAUDE.md project guidelines
- docs/DOJO.md core principles
