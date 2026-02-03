//! Partitioning operations (wipefs/parted).

use crate::HalResult;
use std::path::Path;

#[derive(Debug, Clone)]
pub struct WipeFsOptions {
    pub dry_run: bool,
    pub confirmed: bool,
}

impl WipeFsOptions {
    pub fn new(dry_run: bool, confirmed: bool) -> Self {
        Self { dry_run, confirmed }
    }
}

#[derive(Debug, Clone)]
pub struct PartedOptions {
    pub dry_run: bool,
    pub confirmed: bool,
}

impl PartedOptions {
    pub fn new(dry_run: bool, confirmed: bool) -> Self {
        Self { dry_run, confirmed }
    }
}

/// A high-level partition operation executed via `parted -s`.
///
/// This intentionally stays close to the existing shell-driven flow to keep behavior stable.
#[derive(Debug, Clone)]
pub enum PartedOp {
    MkLabel {
        label: String,
    },
    MkPart {
        part_type: String,
        fs_type: String,
        start: String,
        end: String,
    },
    SetFlag {
        part_num: u32,
        flag: String,
        state: String,
    },
    Print,
}

pub trait PartitionOps {
    fn wipefs_all(&self, disk: &Path, opts: &WipeFsOptions) -> HalResult<()>;

    /// Execute a single `parted` operation on the given disk.
    fn parted(&self, disk: &Path, op: PartedOp, opts: &PartedOptions) -> HalResult<String>;
}
