//! MASH Hardware Abstraction Layer (HAL).
//!
//! Phase 1 scaffold for WO-020 (Grand Refactor).
//! This crate intentionally provides only minimal public interfaces.

use anyhow::Result;

/// Minimal interface for platform / system interactions.
///
/// Future implementations should provide concrete backends (Linux, mock, etc.).
pub trait Hal: Send + Sync {
    fn ensure_root(&self) -> Result<()>;
}

/// A no-op HAL used for compile-time wiring and tests.
#[derive(Debug, Default, Clone, Copy)]
pub struct NoopHal;

impl Hal for NoopHal {
    fn ensure_root(&self) -> Result<()> {
        Ok(())
    }
}
