//! btrfs operations used by the Fedora full-loop installer.

use crate::HalResult;
use std::path::Path;

pub trait BtrfsOps {
    /// List btrfs subvolumes under `mount_point`.
    fn btrfs_subvolume_list(&self, mount_point: &Path) -> HalResult<String>;

    /// Create a btrfs subvolume at `path`.
    fn btrfs_subvolume_create(&self, path: &Path) -> HalResult<()>;
}
