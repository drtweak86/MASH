//! Native file copy operations used in place of rsync.

use crate::HalResult;
use std::path::Path;

/// Options controlling how trees are copied.
#[derive(Debug, Clone)]
pub struct CopyOptions {
    /// Preserve ownership (uid/gid) when supported.
    pub preserve_owner: bool,
    /// Preserve permissions (mode bits).
    pub preserve_perms: bool,
    /// Preserve access and modification times.
    pub preserve_times: bool,
}

impl CopyOptions {
    /// Archive-style copy (similar to `rsync -a`).
    pub fn archive() -> Self {
        Self {
            preserve_owner: true,
            preserve_perms: true,
            preserve_times: true,
        }
    }

    /// VFAT-safe copy (no owner/perms/times).
    pub fn vfat_safe() -> Self {
        Self {
            preserve_owner: false,
            preserve_perms: false,
            preserve_times: false,
        }
    }
}

/// Progress information for a copy operation.
#[derive(Debug, Clone, Default)]
pub struct CopyProgress {
    pub bytes_copied: u64,
    pub bytes_total: u64,
    pub files_copied: u64,
    pub files_total: u64,
}

/// Copy operations abstraction.
pub trait CopyOps {
    /// Recursively copy a directory tree from `src` into `dst`, reporting progress.
    ///
    /// The callback returns `true` to continue or `false` to abort the copy.
    fn copy_tree_native(
        &self,
        src: &Path,
        dst: &Path,
        opts: &CopyOptions,
        on_progress: &mut dyn FnMut(CopyProgress) -> bool,
    ) -> HalResult<()>;
}
