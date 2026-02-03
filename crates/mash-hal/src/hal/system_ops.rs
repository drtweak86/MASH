//! System-level operations (sync, udev settle, etc).

use anyhow::Result;

/// System operations trait.
pub trait SystemOps {
    /// Best-effort filesystem sync.
    fn sync(&self) -> Result<()>;

    /// Best-effort udev settle (wait for block device events to quiesce).
    fn udev_settle(&self) -> Result<()>;
}
