//! OS-specific partitioning rules for full-disk image installs.
//!
//! For Ubuntu / Raspberry Pi OS / Manjaro we flash upstream full-disk images.
//! That means we do NOT create a new partition table during install.
//! The rules here are used to decide whether additional partitions (like a data partition)
//! are allowed or must be skipped.

use crate::downloader::OsKind;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DataPartitionPolicy {
    /// Installer may create a third (data) partition.
    Allowed,
    /// Installer must NOT create a third (data) partition.
    Forbidden,
}

pub fn data_partition_policy(os: OsKind) -> DataPartitionPolicy {
    match os {
        // Fedora is handled by the Fedora-specific installer pipeline which creates its own layout.
        OsKind::Fedora => DataPartitionPolicy::Allowed,
        // Ubuntu images can be used as-is; a data partition may be added but is not required.
        OsKind::Ubuntu => DataPartitionPolicy::Allowed,
        // Raspberry Pi OS uses its own expected layout; leave unchanged.
        OsKind::RaspberryPiOS => DataPartitionPolicy::Forbidden,
        // Non-negotiable per issue #84: Manjaro images ship with 2 partitions; do not add a third.
        OsKind::Manjaro => DataPartitionPolicy::Forbidden,
    }
}
