//! System-level operations (sync, udev settle, etc).

use crate::HalResult;

/// System operations trait.
pub trait SystemOps {
    /// Best-effort filesystem sync.
    fn sync(&self) -> HalResult<()>;

    /// Best-effort udev settle (wait for block device events to quiesce).
    fn udev_settle(&self) -> HalResult<()>;
}
