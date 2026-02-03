//! Process helpers with explicit timeouts (WO-036.7).
//!
//! MASH runs in privileged contexts; external commands must not be allowed to hang indefinitely.

use anyhow::{Context, Result};
use std::io::Read;
use std::process::{Command, ExitStatus, Output, Stdio};
use std::time::Duration;
use wait_timeout::ChildExt;

pub fn status_with_timeout(
    program: &str,
    cmd: &mut Command,
    timeout: Duration,
) -> Result<ExitStatus> {
    // Avoid commands hanging waiting for input.
    cmd.stdin(Stdio::null());
    let mut child = cmd
        .spawn()
        .with_context(|| format!("failed to spawn {}", program))?;

    match child.wait_timeout(timeout).context("wait_timeout failed")? {
        Some(status) => Ok(status),
        None => {
            let _ = child.kill();
            let _ = child.wait();
            anyhow::bail!("{} timed out after {}s", program, timeout.as_secs());
        }
    }
}

pub fn output_with_timeout(program: &str, cmd: &mut Command, timeout: Duration) -> Result<Output> {
    cmd.stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = cmd
        .spawn()
        .with_context(|| format!("failed to spawn {}", program))?;

    let mut stdout = child.stdout.take();
    let mut stderr = child.stderr.take();
    let stdout_handle = std::thread::spawn(move || {
        let mut buf = Vec::new();
        if let Some(mut out) = stdout.take() {
            let _ = out.read_to_end(&mut buf);
        }
        buf
    });
    let stderr_handle = std::thread::spawn(move || {
        let mut buf = Vec::new();
        if let Some(mut err) = stderr.take() {
            let _ = err.read_to_end(&mut buf);
        }
        buf
    });

    let status = match child.wait_timeout(timeout).context("wait_timeout failed")? {
        Some(status) => status,
        None => {
            let _ = child.kill();
            let _ = child.wait();
            let _ = stdout_handle.join();
            let _ = stderr_handle.join();
            anyhow::bail!("{} timed out after {}s", program, timeout.as_secs());
        }
    };

    let stdout = stdout_handle.join().unwrap_or_default();
    let stderr = stderr_handle.join().unwrap_or_default();
    Ok(Output {
        status,
        stdout,
        stderr,
    })
}
