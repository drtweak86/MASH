//! Loop device operations (losetup).

use crate::HalResult;
use std::path::Path;

pub trait LoopOps {
    /// Setup a loop device for the given image, returning the loop path (e.g. `/dev/loop7`).
    ///
    /// If `scan_partitions` is true, the loop device is created with partition scanning (equivalent
    /// to `losetup -P`).
    fn losetup_attach(&self, image: &Path, scan_partitions: bool) -> HalResult<String>;

    /// Detach a loop device.
    fn losetup_detach(&self, loop_device: &str) -> HalResult<()>;
}
