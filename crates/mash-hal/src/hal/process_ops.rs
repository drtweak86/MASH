//! Process execution helpers.
//!
//! External commands are considered "world-touching" and must go through the HAL so we can
//! test workflows without spawning real processes.

use crate::HalResult;
use std::path::Path;
use std::process::Output;
use std::time::Duration;

/// Process execution trait (external command runner).
pub trait ProcessOps {
    fn command_output_with_cwd(
        &self,
        program: &str,
        args: &[&str],
        cwd: Option<&Path>,
        timeout: Duration,
    ) -> HalResult<Output>;

    fn command_status_with_cwd(
        &self,
        program: &str,
        args: &[&str],
        cwd: Option<&Path>,
        timeout: Duration,
    ) -> HalResult<()>;

    fn command_output(&self, program: &str, args: &[&str], timeout: Duration) -> HalResult<Output> {
        self.command_output_with_cwd(program, args, None, timeout)
    }

    fn command_status(&self, program: &str, args: &[&str], timeout: Duration) -> HalResult<()> {
        self.command_status_with_cwd(program, args, None, timeout)
    }
}
