use std::io;
use thiserror::Error;

pub type HalResult<T> = Result<T, HalError>;
pub type MashResult<T> = Result<T, MashError>;

#[derive(Error, Debug)]
pub enum HalError {
    #[error("Safety lock engaged (SAFE MODE). Disarm Safe Mode to proceed.")]
    SafetyLock,

    #[error("Disk is busy (mounted or in use)")]
    DiskBusy,

    #[error("Permission denied")]
    PermissionDenied,

    #[error("Validation failed: {0}")]
    ValidationFailed(String),

    #[error("Command not found: {0}")]
    CommandNotFound(String),

    #[error("Command failed: {program} (exit={code:?}): {stderr}")]
    CommandFailed {
        program: String,
        code: Option<i32>,
        stderr: String,
    },

    #[error("Command timed out: {program} after {timeout_secs}s")]
    CommandTimeout { program: String, timeout_secs: u64 },

    #[error("I/O error: {0}")]
    Io(#[from] io::Error),

    #[error("nix errno: {0}")]
    Nix(#[from] nix::errno::Errno),

    #[error("UTF-8 decode error: {0}")]
    Utf8(#[from] std::string::FromUtf8Error),

    #[error("Parse error: {0}")]
    Parse(String),

    #[error("{0}")]
    Other(String),
}

#[derive(Error, Debug)]
pub enum MashError {
    #[error(transparent)]
    Hal(#[from] HalError),

    #[error("I/O error: {0}")]
    Io(#[from] io::Error),

    #[error("Missing --yes-i-know flag. This operation is destructive!")]
    MissingYesIKnow,

    #[error("Safe Mode is still armed. Disarm Safe Mode to proceed.")]
    MissingSafeModeDisarm,

    #[error("Missing required typed confirmation for execute-mode.")]
    MissingExecuteConfirmation,

    #[error("Validation failed: {0}")]
    ValidationFailed(String),

    #[allow(dead_code)]
    #[error("Operation aborted by user")]
    Aborted,

    #[allow(dead_code)]
    #[error("Command failed: {0}")]
    CommandFailed(String),
}
