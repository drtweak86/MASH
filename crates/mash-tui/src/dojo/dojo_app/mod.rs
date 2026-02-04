//! New application state machine for the single-screen TUI

#![allow(dead_code)]

mod app;
mod input;
mod steps;
mod types;

pub use app::App;
pub use input::InputResult;
pub use steps::InstallStepType;
pub use types::{CustomizeField, DiskOption};
