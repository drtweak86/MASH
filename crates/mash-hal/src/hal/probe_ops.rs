//! Device probing operations (lsblk/blkid).

use crate::HalResult;
use std::path::{Path, PathBuf};

/// Probing operations trait.
pub trait ProbeOps {
    /// Return mountpoints for partitions on the given disk (best-effort).
    fn lsblk_mountpoints(&self, disk: &Path) -> HalResult<Vec<PathBuf>>;

    /// Return a human-readable lsblk table for diagnostics.
    fn lsblk_table(&self, disk: &Path) -> HalResult<String>;

    /// Return UUID for a block device (e.g. `/dev/sda1`).
    fn blkid_uuid(&self, device: &Path) -> HalResult<String>;
}
