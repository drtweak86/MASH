//! Mount operations trait.

use anyhow::Result;
use std::path::Path;

/// Trait for mounting and unmounting filesystems.
pub trait MountOps {
    /// Mount a device to a target path.
    ///
    /// # Arguments
    /// * `device` - Device path (e.g., `/dev/sda1`)
    /// * `target` - Mount point path
    /// * `fstype` - Optional filesystem type (e.g., `"ext4"`, `"vfat"`)
    /// * `options` - Mount options
    /// * `dry_run` - If true, log the operation but don't execute it
    fn mount_device(
        &self,
        device: &Path,
        target: &Path,
        fstype: Option<&str>,
        options: MountOptions,
        dry_run: bool,
    ) -> Result<()>;

    /// Unmount a filesystem.
    ///
    /// # Arguments
    /// * `target` - Mount point path to unmount
    /// * `dry_run` - If true, log the operation but don't execute it
    fn unmount(&self, target: &Path, dry_run: bool) -> Result<()>;

    /// Check if a path is currently mounted.
    ///
    /// # Arguments
    /// * `path` - Path to check
    fn is_mounted(&self, path: &Path) -> Result<bool>;
}

/// Mount options and flags.
#[derive(Debug, Clone, Default)]
pub struct MountOptions {
    /// Additional mount options as a comma-separated string (e.g., "ro,noexec")
    pub options: Option<String>,
}

impl MountOptions {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_options(options: impl Into<String>) -> Self {
        Self {
            options: Some(options.into()),
        }
    }
}
