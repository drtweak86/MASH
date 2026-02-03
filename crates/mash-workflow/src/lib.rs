//! MASH workflow orchestration.
//!
//! This crate holds deterministic workflow execution primitives (stage runner,
//! resumable state progression, etc.). Higher-level crates provide the concrete
//! state type and persistence backend.

pub mod stage_runner;
