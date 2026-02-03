//! MASH TUI.
//!
//! Phase 1 scaffold for WO-020 (Grand Refactor).
//! This crate intentionally provides only minimal public interfaces.

use anyhow::Result;
use mash_workflow::Workflow;

/// Minimal TUI entrypoint interface.
pub trait Tui: Send + Sync {
    fn run(&mut self) -> Result<()>;
}

/// No-op TUI used for compile-time wiring.
#[derive(Debug)]
pub struct NoopTui<W: Workflow> {
    workflow: W,
}

impl<W: Workflow> NoopTui<W> {
    pub fn new(workflow: W) -> Self {
        Self { workflow }
    }
}

impl<W: Workflow> Tui for NoopTui<W> {
    fn run(&mut self) -> Result<()> {
        self.workflow.run()?;
        Ok(())
    }
}
