//! Copy operations trait used for testing via FakeHal.
use crate::HalResult;
use std::path::Path;

#[derive(Debug, Clone)]
pub struct CopyOptions;

impl CopyOptions {
    pub fn archive() -> Self {
        CopyOptions
    }
}

#[derive(Debug, Clone, Default)]
pub struct CopyProgress {
    pub bytes_copied: u64,
    pub bytes_total: u64,
    pub files_copied: u64,
    pub files_total: u64,
}

pub trait CopyOps {
    fn copy_tree_native(
        &self,
        src: &Path,
        dst: &Path,
        opts: &CopyOptions,
        on_progress: &mut dyn FnMut(CopyProgress) -> bool,
    ) -> HalResult<()>;
}
