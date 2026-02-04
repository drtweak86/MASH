//! MASH Hardware Abstraction Layer (HAL).
//!
//! This crate is the boundary for "world-touching" code (Linux `/proc`, `/sys`,
//! filesystem interactions, etc.) and the parsing helpers that support those operations.
//!
//! # HAL Traits
//!
//! The HAL provides traits for system operations that can be implemented by:
//! - `LinuxHal`: Real implementation for production use
//! - `FakeHal`: Mock implementation for CI-safe testing

pub mod os_release;
pub mod path;
pub mod procfs;
pub mod sysfs;

pub mod error;

pub mod hal;
pub use hal::*;

pub use error::{HalError, HalResult};

// Re-export commonly used types
pub use procfs::mountinfo::MountInfo;
