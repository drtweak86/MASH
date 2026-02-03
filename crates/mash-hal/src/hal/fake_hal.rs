//! Fake HAL implementation for testing.
//!
//! This implementation records all operations without executing them,
//! allowing for CI-safe testing without root privileges or real hardware.

use super::{FlashOps, FormatOps, FormatOptions, MountOps, MountOptions};
use anyhow::Result;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

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
    FlashImage {
        image: PathBuf,
        target: PathBuf,
    },
}

/// Shared state for FakeHal operations.
#[derive(Debug, Clone, Default)]
struct FakeHalState {
    /// All operations that were recorded
    operations: Vec<Operation>,
    /// Currently mounted paths
    mounted_paths: HashSet<PathBuf>,
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
}

impl MountOps for FakeHal {
    fn mount_device(
        &self,
        device: &Path,
        target: &Path,
        fstype: Option<&str>,
        _options: MountOptions,
        dry_run: bool,
    ) -> Result<()> {
        if dry_run {
            log::info!(
                "FAKE HAL DRY RUN: mount {} -> {}",
                device.display(),
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

    fn unmount(&self, target: &Path, dry_run: bool) -> Result<()> {
        if dry_run {
            log::info!("FAKE HAL DRY RUN: unmount {}", target.display());
            return Ok(());
        }

        log::info!("FAKE HAL: unmount {}", target.display());

        self.record_operation(Operation::Unmount {
            target: target.to_path_buf(),
        });
        self.record_unmount(target);

        Ok(())
    }

    fn is_mounted(&self, path: &Path) -> Result<bool> {
        let is_mounted = self.state.lock().unwrap().mounted_paths.contains(path);
        log::info!("FAKE HAL: is_mounted({}) = {}", path.display(), is_mounted);
        Ok(is_mounted)
    }
}

impl FormatOps for FakeHal {
    fn format_ext4(&self, device: &Path, opts: &FormatOptions) -> Result<()> {
        if !opts.dry_run && !opts.confirmed {
            return Err(anyhow::Error::new(crate::HalError::SafetyLock));
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

    fn format_btrfs(&self, device: &Path, opts: &FormatOptions) -> Result<()> {
        if !opts.dry_run && !opts.confirmed {
            return Err(anyhow::Error::new(crate::HalError::SafetyLock));
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
}

impl FlashOps for FakeHal {
    fn flash_raw_image(
        &self,
        image_path: &Path,
        target_disk: &Path,
        opts: &crate::FlashOptions,
    ) -> Result<()> {
        if !opts.dry_run && !opts.confirmed {
            return Err(anyhow::Error::new(crate::HalError::SafetyLock));
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
        assert!(err.downcast_ref::<crate::HalError>().is_some());

        let err = hal.format_btrfs(Path::new("/dev/sda2"), &opts).unwrap_err();
        assert!(err.downcast_ref::<crate::HalError>().is_some());

        let flash_opts = crate::FlashOptions::new(false, false);
        let err = hal
            .flash_raw_image(
                Path::new("/tmp/image.img"),
                Path::new("/dev/sda"),
                &flash_opts,
            )
            .unwrap_err();
        assert!(err.downcast_ref::<crate::HalError>().is_some());
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
}
