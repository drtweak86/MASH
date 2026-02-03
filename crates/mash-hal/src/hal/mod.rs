//! HAL trait definitions and implementations.
//!
//! This module defines the core traits for system operations and provides
//! both real (LinuxHal) and fake (FakeHal) implementations.

pub mod fake_hal;
pub mod flash_ops;
pub mod format_ops;
pub mod linux_hal;
pub mod mount_ops;

pub use fake_hal::{FakeHal, Operation};
pub use flash_ops::{FlashOps, FlashOptions};
pub use format_ops::{FormatOps, FormatOptions};
pub use linux_hal::LinuxHal;
pub use mount_ops::{MountOps, MountOptions};

/// Complete HAL combining all system operation traits.
pub trait SystemHal: MountOps + FormatOps + FlashOps + Send + Sync {}

/// Automatically implement SystemHal for any type implementing all required traits.
impl<T> SystemHal for T where T: MountOps + FormatOps + FlashOps + Send + Sync {}
