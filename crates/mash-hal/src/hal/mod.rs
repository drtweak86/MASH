//! HAL trait definitions and implementations.
//!
//! This module defines the core traits for system operations and provides
//! both real (LinuxHal) and fake (FakeHal) implementations.

pub mod btrfs_ops;
pub mod copy_ops;
pub mod fake_hal;
pub mod flash_ops;
pub mod format_ops;
pub mod host_info_ops;
pub mod linux_hal;
pub mod loop_ops;
pub mod mount_ops;
pub mod partition_ops;
pub mod probe_ops;
pub mod process_ops;
pub mod rsync_ops;
pub mod system_ops;

pub use btrfs_ops::BtrfsOps;
pub use copy_ops::{CopyOps, CopyOptions, CopyProgress};
pub use fake_hal::{FakeHal, Operation};
pub use flash_ops::{FlashOps, FlashOptions};
pub use format_ops::{FormatOps, FormatOptions};
pub use host_info_ops::{HostInfoOps, OsReleaseInfo};
pub use linux_hal::LinuxHal;
pub use loop_ops::LoopOps;
pub use mount_ops::{MountOps, MountOptions};
pub use partition_ops::{PartedOp, PartedOptions, PartitionOps, WipeFsOptions};
pub use probe_ops::ProbeOps;
pub use process_ops::ProcessOps;
pub use rsync_ops::{RsyncOps, RsyncOptions};
pub use system_ops::SystemOps;

/// Complete HAL combining all system operation traits.
pub trait SystemHal: MountOps + FormatOps + FlashOps + Send + Sync {}

/// Automatically implement SystemHal for any type implementing all required traits.
impl<T> SystemHal for T where T: MountOps + FormatOps + FlashOps + Send + Sync {}

/// Installer HAL for full-loop workflows that need partitioning, probing, loop devices,
/// filesystem tooling, and high-level copy operations.
pub trait InstallerHal:
    SystemHal
    + SystemOps
    + HostInfoOps
    + ProcessOps
    + ProbeOps
    + PartitionOps
    + LoopOps
    + BtrfsOps
    + RsyncOps
    + Send
    + Sync
{
}

impl<T> InstallerHal for T where
    T: SystemHal
        + SystemOps
        + HostInfoOps
        + ProcessOps
        + ProbeOps
        + PartitionOps
        + LoopOps
        + BtrfsOps
        + RsyncOps
        + Send
        + Sync
{
}
