//! Filesystem formatting operations trait.

use anyhow::Result;
use std::path::Path;

/// Trait for formatting block devices.
pub trait FormatOps {
    /// Format a device with ext4 filesystem.
    ///
    /// # Arguments
    /// * `device` - Block device path (e.g., `/dev/sda1`)
    /// * `opts` - Formatting options including dry-run and confirmation
    fn format_ext4(&self, device: &Path, opts: &FormatOptions) -> Result<()>;

    /// Format a device with btrfs filesystem.
    ///
    /// # Arguments
    /// * `device` - Block device path (e.g., `/dev/sda2`)
    /// * `opts` - Formatting options including dry-run and confirmation
    fn format_btrfs(&self, device: &Path, opts: &FormatOptions) -> Result<()>;

    /// Format a device with VFAT (FAT32), typically used for EFI system partitions.
    fn format_vfat(&self, device: &Path, label: &str, opts: &FormatOptions) -> Result<()>;
}

/// Options for formatting operations.
#[derive(Debug, Clone)]
pub struct FormatOptions {
    /// If true, log the operation but don't execute it
    pub dry_run: bool,
    /// If true, the user has confirmed the destructive operation
    pub confirmed: bool,
    /// Additional arguments to pass to the format command
    pub extra_args: Vec<String>,
}

impl FormatOptions {
    pub fn new(dry_run: bool, confirmed: bool) -> Self {
        Self {
            dry_run,
            confirmed,
            extra_args: Vec::new(),
        }
    }

    pub fn with_args(mut self, args: Vec<String>) -> Self {
        self.extra_args = args;
        self
    }
}
