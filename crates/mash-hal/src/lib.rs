//! MASH Hardware Abstraction Layer (HAL).
//!
//! This crate is the boundary for "world-touching" code (Linux `/proc`, `/sys`,
//! filesystem interactions, etc.) and the parsing helpers that support those operations.

pub mod os_release;
pub mod procfs;
pub mod sysfs;

// HAL backends (Linux, mock) will be introduced as WO-020 progresses.
