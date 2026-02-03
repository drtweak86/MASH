//! MASH workflow orchestration.
//!
//! Phase 1 scaffold for WO-020 (Grand Refactor).
//! This crate intentionally provides only minimal public interfaces.

use anyhow::Result;
use mash_hal::Hal;

/// A minimal workflow runner interface.
pub trait Workflow: Send + Sync {
    fn run(&self) -> Result<()>;
}

/// A no-op workflow used for compile-time wiring.
#[derive(Debug)]
pub struct NoopWorkflow<H: Hal> {
    hal: H,
}

impl<H: Hal> NoopWorkflow<H> {
    pub fn new(hal: H) -> Self {
        Self { hal }
    }
}

impl<H: Hal> Workflow for NoopWorkflow<H> {
    fn run(&self) -> Result<()> {
        self.hal.ensure_root()?;
        Ok(())
    }
}
