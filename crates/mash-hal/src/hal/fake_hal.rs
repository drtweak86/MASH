//! Fake HAL implementation for testing.
//!
//! This implementation records all operations without executing them,
//! allowing for CI-safe testing without root privileges or real hardware.

use super::{
    BtrfsOps, CopyOps, CopyOptions, CopyProgress, FlashOps, FormatOps, FormatOptions, HostInfoOps,
    LoopOps, MountOps, MountOptions, OsReleaseInfo, PartedOp, PartedOptions, PartitionOps,
    ProbeOps, ProcessOps, RsyncOps, RsyncOptions, SystemOps, WipeFsOptions,
};
use crate::{HalError, HalResult};
use nix::libc;
use std::collections::HashSet;
#[cfg(unix)]
use std::os::unix::process::ExitStatusExt;
use std::path::{Path, PathBuf};
use std::process::Output;
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// Operation records for testing and verification.
#[derive(Debug, Clone)]
pub enum Operation {
    Mount {
        device: PathBuf,
        target: PathBuf,
        fstype: Option<String>,
    },
    Unmount {
        target: PathBuf,
    },
    FormatExt4 {
        device: PathBuf,
    },
    FormatBtrfs {
        device: PathBuf,
    },
    FormatVfat {
        device: PathBuf,
        label: String,
    },
    FlashImage {
        image: PathBuf,
        target: PathBuf,
    },
    Sync,
    UdevSettle,
    WipeFsAll {
        disk: PathBuf,
    },
    Parted {
        disk: PathBuf,
        op: String,
    },
    LosetupAttach {
        image: PathBuf,
        scan_partitions: bool,
        loop_device: String,
    },
    LosetupDetach {
        loop_device: String,
    },
    BtrfsSubvolumeList {
        mount_point: PathBuf,
    },
    BtrfsSubvolumeCreate {
        path: PathBuf,
    },
    CopyTree {
        src: PathBuf,
        dst: PathBuf,
    },
    Rsync {
        src: PathBuf,
        dst: PathBuf,
    },
    InjectedFailure(&'static str),
    LsblkMountpoints {
        disk: PathBuf,
    },
    LsblkTable {
        disk: PathBuf,
    },
    BlkidUuid {
        device: PathBuf,
    },
    Command {
        program: String,
        args: Vec<String>,
        timeout_secs: u64,
    },
}

/// Shared state for FakeHal operations.
#[derive(Debug, Clone, Default)]
struct FakeHalState {
    /// All operations that were recorded
    operations: Vec<Operation>,
    /// Currently mounted paths
    mounted_paths: HashSet<PathBuf>,
    /// Optional injected failure for deterministic testing.
    failure: Option<InjectedFailure>,
}

#[derive(Debug, Clone)]
pub enum InjectedFailure {
    DiskFull,
    PermissionDenied,
    MidCopy { after_bytes: u64 },
    OutOfMemory,
}

/// Fake HAL implementation that records operations without executing them.
///
/// This is designed for testing and CI environments where real system
/// operations would fail or be dangerous.
#[derive(Debug, Clone, Default)]
pub struct FakeHal {
    state: Arc<Mutex<FakeHalState>>,
}

impl FakeHal {
    pub fn new() -> Self {
        Self {
            state: Arc::new(Mutex::new(FakeHalState::default())),
        }
    }

    /// Configure a deterministic failure for the next operation.
    pub fn set_failure(&self, failure: InjectedFailure) {
        self.state.lock().unwrap().failure = Some(failure);
    }

    /// Get all recorded operations.
    pub fn operations(&self) -> Vec<Operation> {
        self.state.lock().unwrap().operations.clone()
    }

    /// Get the number of operations recorded.
    pub fn operation_count(&self) -> usize {
        self.state.lock().unwrap().operations.len()
    }

    /// Check if a specific operation was recorded.
    pub fn has_operation(&self, check: impl Fn(&Operation) -> bool) -> bool {
        self.state.lock().unwrap().operations.iter().any(check)
    }

    /// Clear all recorded operations.
    pub fn clear(&self) {
        let mut state = self.state.lock().unwrap();
        state.operations.clear();
        state.mounted_paths.clear();
        state.failure = None;
    }

    /// Simulate a mount by adding it to the mounted set.
    fn record_mount(&self, target: PathBuf) {
        self.state.lock().unwrap().mounted_paths.insert(target);
    }

    /// Simulate an unmount by removing it from the mounted set.
    fn record_unmount(&self, target: &Path) {
        self.state.lock().unwrap().mounted_paths.remove(target);
    }

    fn record_operation(&self, op: Operation) {
        self.state.lock().unwrap().operations.push(op);
    }

    fn take_failure(&self) -> Option<InjectedFailure> {
        self.state.lock().unwrap().failure.take()
    }

    fn take_failure_if(&self, pred: impl Fn(&InjectedFailure) -> bool) -> bool {
        let mut state = self.state.lock().unwrap();
        if state.failure.as_ref().is_some_and(&pred) {
            state.failure.take();
            true
        } else {
            false
        }
    }
}

impl HostInfoOps for FakeHal {
    fn hostname(&self) -> HalResult<Option<String>> {
        Ok(None)
    }

    fn kernel_release(&self) -> HalResult<Option<String>> {
        Ok(None)
    }

    fn os_release(&self) -> HalResult<OsReleaseInfo> {
        Ok(OsReleaseInfo {
            id: None,
            version_id: None,
        })
    }

    fn proc_cmdline(&self) -> HalResult<String> {
        Ok(String::new())
    }

    fn proc_cpuinfo(&self) -> HalResult<String> {
        Ok(String::new())
    }

    fn proc_meminfo(&self) -> HalResult<String> {
        Ok(String::new())
    }

    fn proc_mounts(&self) -> HalResult<String> {
        Ok(String::new())
    }

    fn proc_mountinfo(&self) -> HalResult<String> {
        Ok(String::new())
    }
}

impl ProcessOps for FakeHal {
    fn command_output_with_cwd(
        &self,
        program: &str,
        args: &[&str],
        _cwd: Option<&Path>,
        timeout: Duration,
    ) -> HalResult<Output> {
        self.record_operation(Operation::Command {
            program: program.to_string(),
            args: args.iter().map(|s| s.to_string()).collect(),
            timeout_secs: timeout.as_secs(),
        });
        #[cfg(unix)]
        let status = std::process::ExitStatus::from_raw(0);
        #[cfg(not(unix))]
        let status = std::process::Command::new("true").status().unwrap();

        Ok(Output {
            status,
            stdout: Vec::new(),
            stderr: Vec::new(),
        })
    }

    fn command_status_with_cwd(
        &self,
        program: &str,
        args: &[&str],
        cwd: Option<&Path>,
        timeout: Duration,
    ) -> HalResult<()> {
        let _ = self.command_output_with_cwd(program, args, cwd, timeout)?;
        Ok(())
    }
}

impl MountOps for FakeHal {
    fn mount_device(
        &self,
        device: &Path,
        target: &Path,
        fstype: Option<&str>,
        _options: MountOptions,
        dry_run: bool,
    ) -> HalResult<()> {
        if self.take_failure_if(|f| matches!(f, InjectedFailure::PermissionDenied)) {
            self.record_operation(Operation::InjectedFailure("permission_denied"));
            return Err(HalError::PermissionDenied);
        }
        if dry_run {
            log::info!(
                "FAKE HAL DRY RUN: mount {} -> {}",
                device.display(),
                target.display()
            );
            return Ok(());
        }

        if self.is_mounted(target)? {
            log::info!(
                "FAKE HAL: mount skipped, already mounted -> {}",
                target.display()
            );
            return Ok(());
        }

        log::info!(
            "FAKE HAL: mount {} -> {} (type: {:?})",
            device.display(),
            target.display(),
            fstype
        );

        self.record_operation(Operation::Mount {
            device: device.to_path_buf(),
            target: target.to_path_buf(),
            fstype: fstype.map(String::from),
        });
        self.record_mount(target.to_path_buf());

        Ok(())
    }

    fn unmount(&self, target: &Path, dry_run: bool) -> HalResult<()> {
        if dry_run {
            log::info!("FAKE HAL DRY RUN: unmount {}", target.display());
            return Ok(());
        }

        if !self.is_mounted(target)? {
            log::info!(
                "FAKE HAL: unmount skipped, not mounted -> {}",
                target.display()
            );
            return Ok(());
        }

        log::info!("FAKE HAL: unmount {}", target.display());

        self.record_operation(Operation::Unmount {
            target: target.to_path_buf(),
        });
        self.record_unmount(target);

        Ok(())
    }

    fn unmount_recursive(&self, target: &Path, dry_run: bool) -> HalResult<()> {
        // FakeHal does not model nested mount trees; treat as a normal unmount for recording.
        self.unmount(target, dry_run)
    }

    fn is_mounted(&self, path: &Path) -> HalResult<bool> {
        let is_mounted = self.state.lock().unwrap().mounted_paths.contains(path);
        log::info!("FAKE HAL: is_mounted({}) = {}", path.display(), is_mounted);
        Ok(is_mounted)
    }
}

impl FormatOps for FakeHal {
    fn format_ext4(&self, device: &Path, opts: &FormatOptions) -> HalResult<()> {
        if !opts.dry_run && !opts.confirmed {
            return Err(HalError::SafetyLock);
        }

        if self.take_failure_if(|f| matches!(f, InjectedFailure::PermissionDenied)) {
            self.record_operation(Operation::InjectedFailure("permission_denied"));
            return Err(HalError::PermissionDenied);
        }

        if opts.dry_run {
            log::info!("FAKE HAL DRY RUN: mkfs.ext4 {}", device.display());
            return Ok(());
        }

        log::info!("FAKE HAL: mkfs.ext4 {}", device.display());

        self.record_operation(Operation::FormatExt4 {
            device: device.to_path_buf(),
        });

        Ok(())
    }

    fn format_btrfs(&self, device: &Path, opts: &FormatOptions) -> HalResult<()> {
        if !opts.dry_run && !opts.confirmed {
            return Err(HalError::SafetyLock);
        }

        if self.take_failure_if(|f| matches!(f, InjectedFailure::PermissionDenied)) {
            self.record_operation(Operation::InjectedFailure("permission_denied"));
            return Err(HalError::PermissionDenied);
        }

        if opts.dry_run {
            log::info!("FAKE HAL DRY RUN: mkfs.btrfs {}", device.display());
            return Ok(());
        }

        log::info!("FAKE HAL: mkfs.btrfs {}", device.display());

        self.record_operation(Operation::FormatBtrfs {
            device: device.to_path_buf(),
        });

        Ok(())
    }

    fn format_vfat(&self, device: &Path, label: &str, opts: &FormatOptions) -> HalResult<()> {
        if !opts.dry_run && !opts.confirmed {
            return Err(HalError::SafetyLock);
        }

        if self.take_failure_if(|f| matches!(f, InjectedFailure::PermissionDenied)) {
            self.record_operation(Operation::InjectedFailure("permission_denied"));
            return Err(HalError::PermissionDenied);
        }

        if opts.dry_run {
            log::info!(
                "FAKE HAL DRY RUN: mkfs.vfat {} ({})",
                device.display(),
                label
            );
            return Ok(());
        }

        log::info!("FAKE HAL: mkfs.vfat {} ({})", device.display(), label);

        self.record_operation(Operation::FormatVfat {
            device: device.to_path_buf(),
            label: label.to_string(),
        });

        Ok(())
    }
}

impl FlashOps for FakeHal {
    fn flash_raw_image(
        &self,
        image_path: &Path,
        target_disk: &Path,
        opts: &crate::FlashOptions,
    ) -> HalResult<()> {
        if !opts.dry_run && !opts.confirmed {
            return Err(HalError::SafetyLock);
        }

        if opts.dry_run {
            log::info!(
                "FAKE HAL DRY RUN: flash {} -> {}",
                image_path.display(),
                target_disk.display()
            );
            return Ok(());
        }

        log::info!(
            "FAKE HAL: flash {} -> {}",
            image_path.display(),
            target_disk.display()
        );

        self.record_operation(Operation::FlashImage {
            image: image_path.to_path_buf(),
            target: target_disk.to_path_buf(),
        });

        Ok(())
    }
}

impl SystemOps for FakeHal {
    fn sync(&self) -> HalResult<()> {
        self.record_operation(Operation::Sync);
        Ok(())
    }

    fn udev_settle(&self) -> HalResult<()> {
        self.record_operation(Operation::UdevSettle);
        Ok(())
    }
}

impl ProbeOps for FakeHal {
    fn lsblk_mountpoints(&self, disk: &Path) -> HalResult<Vec<PathBuf>> {
        self.record_operation(Operation::LsblkMountpoints {
            disk: disk.to_path_buf(),
        });
        Ok(Vec::new())
    }

    fn lsblk_table(&self, disk: &Path) -> HalResult<String> {
        self.record_operation(Operation::LsblkTable {
            disk: disk.to_path_buf(),
        });
        Ok(String::new())
    }

    fn blkid_uuid(&self, device: &Path) -> HalResult<String> {
        self.record_operation(Operation::BlkidUuid {
            device: device.to_path_buf(),
        });
        Ok("FAKE-UUID".to_string())
    }
}

impl PartitionOps for FakeHal {
    fn wipefs_all(&self, disk: &Path, opts: &WipeFsOptions) -> HalResult<()> {
        if !opts.dry_run && !opts.confirmed {
            return Err(HalError::SafetyLock);
        }
        self.record_operation(Operation::WipeFsAll {
            disk: disk.to_path_buf(),
        });
        Ok(())
    }

    fn parted(&self, disk: &Path, op: PartedOp, opts: &PartedOptions) -> HalResult<String> {
        if !opts.dry_run && !opts.confirmed {
            return Err(HalError::SafetyLock);
        }
        self.record_operation(Operation::Parted {
            disk: disk.to_path_buf(),
            op: format!("{:?}", op),
        });
        Ok(String::new())
    }
}

impl LoopOps for FakeHal {
    fn losetup_attach(&self, image: &Path, scan_partitions: bool) -> HalResult<String> {
        let loop_device = "/dev/loop0".to_string();
        self.record_operation(Operation::LosetupAttach {
            image: image.to_path_buf(),
            scan_partitions,
            loop_device: loop_device.clone(),
        });
        Ok(loop_device)
    }

    fn losetup_detach(&self, loop_device: &str) -> HalResult<()> {
        self.record_operation(Operation::LosetupDetach {
            loop_device: loop_device.to_string(),
        });
        Ok(())
    }
}

impl BtrfsOps for FakeHal {
    fn btrfs_subvolume_list(&self, mount_point: &Path) -> HalResult<String> {
        self.record_operation(Operation::BtrfsSubvolumeList {
            mount_point: mount_point.to_path_buf(),
        });
        Ok(String::new())
    }

    fn btrfs_subvolume_create(&self, path: &Path) -> HalResult<()> {
        self.record_operation(Operation::BtrfsSubvolumeCreate {
            path: path.to_path_buf(),
        });
        Ok(())
    }
}

impl CopyOps for FakeHal {
    fn copy_tree_native(
        &self,
        src: &Path,
        dst: &Path,
        _opts: &CopyOptions,
        on_progress: &mut dyn FnMut(CopyProgress) -> bool,
    ) -> HalResult<()> {
        if let Some(failure) = self.take_failure() {
            match failure {
                InjectedFailure::DiskFull => {
                    self.record_operation(Operation::InjectedFailure("disk_full"));
                    return Err(HalError::Io(std::io::Error::from_raw_os_error(
                        libc::ENOSPC,
                    )));
                }
                InjectedFailure::PermissionDenied => {
                    self.record_operation(Operation::InjectedFailure("permission_denied"));
                    return Err(HalError::PermissionDenied);
                }
                InjectedFailure::MidCopy { after_bytes } => {
                    self.record_operation(Operation::InjectedFailure("mid_copy"));
                    let progress = CopyProgress {
                        bytes_copied: after_bytes,
                        bytes_total: after_bytes.saturating_add(1),
                        files_copied: 1,
                        files_total: 2,
                    };
                    let _ = on_progress(progress);
                    return Err(HalError::Io(std::io::Error::from(
                        std::io::ErrorKind::WriteZero,
                    )));
                }
                InjectedFailure::OutOfMemory => {
                    self.record_operation(Operation::InjectedFailure("out_of_memory"));
                    return Err(HalError::Other("out of memory".to_string()));
                }
            }
        }

        self.record_operation(Operation::CopyTree {
            src: src.to_path_buf(),
            dst: dst.to_path_buf(),
        });
        let _ = on_progress(CopyProgress::default());
        Ok(())
    }
}

impl RsyncOps for FakeHal {
    fn rsync_stream_stdout(
        &self,
        src: &Path,
        dst: &Path,
        _opts: &RsyncOptions,
        _on_stdout_line: &mut dyn FnMut(&str) -> bool,
    ) -> HalResult<()> {
        self.record_operation(Operation::Rsync {
            src: src.to_path_buf(),
            dst: dst.to_path_buf(),
        });
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fake_hal_records_mount() {
        let hal = FakeHal::new();
        let device = Path::new("/dev/sda1");
        let target = Path::new("/mnt/test");

        hal.mount_device(device, target, Some("ext4"), MountOptions::new(), false)
            .unwrap();

        assert_eq!(hal.operation_count(), 1);
        assert!(hal.has_operation(|op| matches!(op, Operation::Mount { .. })));
        assert!(hal.is_mounted(target).unwrap());
    }

    #[test]
    fn fake_hal_records_unmount() {
        let hal = FakeHal::new();
        let target = Path::new("/mnt/test");

        // Mount first
        hal.mount_device(
            Path::new("/dev/sda1"),
            target,
            Some("ext4"),
            MountOptions::new(),
            false,
        )
        .unwrap();

        // Then unmount
        hal.unmount(target, false).unwrap();

        assert_eq!(hal.operation_count(), 2);
        assert!(hal.has_operation(|op| matches!(op, Operation::Unmount { .. })));
        assert!(!hal.is_mounted(target).unwrap());
    }

    #[test]
    fn fake_hal_mount_unmount_idempotent() {
        let hal = FakeHal::new();
        let target = Path::new("/mnt/idempotent");

        hal.mount_device(
            Path::new("/dev/sdb1"),
            target,
            Some("ext4"),
            MountOptions::new(),
            false,
        )
        .unwrap();
        hal.mount_device(
            Path::new("/dev/sdb1"),
            target,
            Some("ext4"),
            MountOptions::new(),
            false,
        )
        .unwrap();
        hal.unmount(target, false).unwrap();
        hal.unmount(target, false).unwrap();

        assert_eq!(hal.operation_count(), 2);
        assert!(!hal.is_mounted(target).unwrap());
    }

    #[test]
    fn fake_hal_records_format_ext4() {
        let hal = FakeHal::new();
        let device = Path::new("/dev/sda1");
        let opts = FormatOptions::new(false, true);

        hal.format_ext4(device, &opts).unwrap();

        assert_eq!(hal.operation_count(), 1);
        assert!(hal.has_operation(|op| matches!(op, Operation::FormatExt4 { .. })));
    }

    #[test]
    fn fake_hal_records_format_btrfs() {
        let hal = FakeHal::new();
        let device = Path::new("/dev/sda2");
        let opts = FormatOptions::new(false, true);

        hal.format_btrfs(device, &opts).unwrap();

        assert_eq!(hal.operation_count(), 1);
        assert!(hal.has_operation(|op| matches!(op, Operation::FormatBtrfs { .. })));
    }

    #[test]
    fn fake_hal_records_flash() {
        let hal = FakeHal::new();
        let image = Path::new("/tmp/image.img");
        let target = Path::new("/dev/sda");

        let opts = crate::FlashOptions::new(false, true);
        hal.flash_raw_image(image, target, &opts).unwrap();

        assert_eq!(hal.operation_count(), 1);
        assert!(hal.has_operation(|op| matches!(op, Operation::FlashImage { .. })));
    }

    #[test]
    fn fake_hal_requires_confirmation() {
        let hal = FakeHal::new();
        let opts = FormatOptions::new(false, false);

        let err = hal.format_ext4(Path::new("/dev/sda1"), &opts).unwrap_err();
        assert!(matches!(err, HalError::SafetyLock));

        let err = hal.format_btrfs(Path::new("/dev/sda2"), &opts).unwrap_err();
        assert!(matches!(err, HalError::SafetyLock));

        let flash_opts = crate::FlashOptions::new(false, false);
        let err = hal
            .flash_raw_image(
                Path::new("/tmp/image.img"),
                Path::new("/dev/sda"),
                &flash_opts,
            )
            .unwrap_err();
        assert!(matches!(err, HalError::SafetyLock));
    }

    #[test]
    fn fake_hal_can_clear() {
        let hal = FakeHal::new();
        hal.format_ext4(Path::new("/dev/sda1"), &FormatOptions::new(false, true))
            .unwrap();

        assert_eq!(hal.operation_count(), 1);

        hal.clear();

        assert_eq!(hal.operation_count(), 0);
    }

    #[test]
    fn injected_disk_full_errors_copy() {
        let hal = FakeHal::new();
        hal.set_failure(InjectedFailure::DiskFull);
        let err = hal
            .copy_tree_native(
                Path::new("/src"),
                Path::new("/dst"),
                &CopyOptions::archive(),
                &mut |_p| true,
            )
            .unwrap_err();
        assert!(matches!(err, HalError::Io(e) if e.raw_os_error() == Some(libc::ENOSPC)));
        assert!(hal.has_operation(|op| matches!(op, Operation::InjectedFailure("disk_full"))));
    }

    #[test]
    fn injected_permission_denied_blocks_format() {
        let hal = FakeHal::new();
        hal.set_failure(InjectedFailure::PermissionDenied);
        let err = hal
            .format_ext4(Path::new("/dev/sdz1"), &FormatOptions::new(false, true))
            .unwrap_err();
        assert!(matches!(err, HalError::PermissionDenied));
        assert!(
            hal.has_operation(|op| matches!(op, Operation::InjectedFailure("permission_denied")))
        );
    }

    #[test]
    fn injected_mid_copy_triggers_write_zero() {
        let hal = FakeHal::new();
        hal.set_failure(InjectedFailure::MidCopy { after_bytes: 1024 });
        let err = hal
            .copy_tree_native(
                Path::new("/src"),
                Path::new("/dst"),
                &CopyOptions::archive(),
                &mut |_p| true,
            )
            .unwrap_err();
        assert!(matches!(err, HalError::Io(e) if e.kind() == std::io::ErrorKind::WriteZero));
    }

    #[test]
    fn injected_oom_returns_error() {
        let hal = FakeHal::new();
        hal.set_failure(InjectedFailure::OutOfMemory);
        let err = hal
            .copy_tree_native(
                Path::new("/src"),
                Path::new("/dst"),
                &CopyOptions::archive(),
                &mut |_p| true,
            )
            .unwrap_err();
        assert!(matches!(err, HalError::Other(msg) if msg.contains("out of memory")));
    }
}
