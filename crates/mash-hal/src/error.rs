use thiserror::Error;

pub type HalResult<T> = std::result::Result<T, HalError>;

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
    Io(#[from] std::io::Error),

    #[error("nix errno: {0}")]
    Nix(#[from] nix::errno::Errno),

    #[error("UTF-8 decode error: {0}")]
    Utf8(#[from] std::string::FromUtf8Error),

    #[error("Parse error: {0}")]
    Parse(String),

    #[error("{0}")]
    Other(String),
}
