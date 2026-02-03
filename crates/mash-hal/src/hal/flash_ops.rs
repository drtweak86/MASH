//! Disk image flashing operations trait.

use crate::HalResult;
use std::path::Path;

/// Options for destructive flash operations.
#[derive(Debug, Clone)]
pub struct FlashOptions {
    pub dry_run: bool,
    pub confirmed: bool,
}

impl FlashOptions {
    pub fn new(dry_run: bool, confirmed: bool) -> Self {
        Self { dry_run, confirmed }
    }
}

/// Trait for flashing disk images to block devices.
pub trait FlashOps {
    /// Flash a raw disk image to a target block device.
    ///
    /// Supports both raw images and `.xz`-compressed images.
    /// Uses streaming decompression for `.xz` files.
    ///
    /// # Arguments
    /// * `image_path` - Path to the disk image file
    /// * `target_disk` - Target block device path (e.g., `/dev/sda`)
    fn flash_raw_image(
        &self,
        image_path: &Path,
        target_disk: &Path,
        opts: &FlashOptions,
    ) -> HalResult<()>;
}
