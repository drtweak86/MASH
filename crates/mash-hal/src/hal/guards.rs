use crate::{LoopOps, MountOps};
use std::path::{Path, PathBuf};

/// RAII guard that unmounts a target path when dropped.
#[derive(Debug)]
pub struct MountGuard<'a, H: MountOps + ?Sized> {
    hal: &'a H,
    target: PathBuf,
    dry_run: bool,
    active: bool,
}

impl<'a, H: MountOps + ?Sized> MountGuard<'a, H> {
    pub fn new(hal: &'a H, target: impl Into<PathBuf>, dry_run: bool) -> Self {
        Self {
            hal,
            target: target.into(),
            dry_run,
            active: true,
        }
    }

    /// Prevent automatic unmounting and return the target path.
    pub fn release(mut self) -> PathBuf {
        self.active = false;
        self.target.clone()
    }

    pub fn target(&self) -> &Path {
        &self.target
    }
}

impl<'a, H: MountOps + ?Sized> Drop for MountGuard<'a, H> {
    fn drop(&mut self) {
        if !self.active {
            return;
        }
        if let Err(err) = self.hal.unmount_recursive(&self.target, self.dry_run) {
            log::warn!(
                "mount guard failed to unmount {}: {}",
                self.target.display(),
                err
            );
        }
    }
}

/// RAII guard that detaches a loop device when dropped.
#[derive(Debug)]
pub struct LoopGuard<'a, H: LoopOps + ?Sized> {
    hal: &'a H,
    loop_device: String,
    active: bool,
}

impl<'a, H: LoopOps + ?Sized> LoopGuard<'a, H> {
    pub fn new(hal: &'a H, loop_device: impl Into<String>) -> Self {
        Self {
            hal,
            loop_device: loop_device.into(),
            active: true,
        }
    }

    /// Prevent automatic detach and return the loop device path.
    pub fn release(mut self) -> String {
        self.active = false;
        self.loop_device.clone()
    }

    pub fn device(&self) -> &str {
        &self.loop_device
    }
}

impl<'a, H: LoopOps + ?Sized> Drop for LoopGuard<'a, H> {
    fn drop(&mut self) {
        if !self.active {
            return;
        }
        if let Err(err) = self.hal.losetup_detach(&self.loop_device) {
            log::warn!("loop guard failed to detach {}: {}", self.loop_device, err);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{FakeHal, MountOptions};
    use std::path::Path;

    #[test]
    fn mount_guard_unmounts_on_drop() {
        let hal = FakeHal::new();
        let target = Path::new("/mnt/test");

        hal.mount_device(
            Path::new("/dev/sda1"),
            target,
            Some("ext4"),
            MountOptions::new(),
            false,
        )
        .unwrap();
        assert!(hal.is_mounted(target).unwrap());

        {
            let _guard = MountGuard::new(&hal, target.to_path_buf(), false);
        }

        assert!(!hal.is_mounted(target).unwrap());
    }

    #[test]
    fn mount_guard_release_skips_unmount() {
        let hal = FakeHal::new();
        let target = Path::new("/mnt/keep");

        hal.mount_device(
            Path::new("/dev/sda2"),
            target,
            Some("ext4"),
            MountOptions::new(),
            false,
        )
        .unwrap();
        assert!(hal.is_mounted(target).unwrap());

        {
            let guard = MountGuard::new(&hal, target.to_path_buf(), false);
            let _ = guard.release();
        }

        assert!(hal.is_mounted(target).unwrap());
    }

    #[test]
    fn loop_guard_detaches_on_drop() {
        let hal = FakeHal::new();
        let loop_dev = hal
            .losetup_attach(Path::new("/tmp/image.img"), true)
            .unwrap();

        {
            let _guard = LoopGuard::new(&hal, loop_dev.clone());
        }

        assert!(hal
            .operations()
            .iter()
            .any(|op| matches!(op, crate::Operation::LosetupDetach { .. })));
    }
}
