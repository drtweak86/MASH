use thiserror::Error;

/// Result type alias for MASH operations
pub type Result<T> = anyhow::Result<T>;

#[derive(Error, Debug)]
pub enum MashError {
    #[error("Missing --yes-i-know flag. This operation is destructive!")]
    MissingYesIKnow,

    #[error("Safety lock engaged (SAFE MODE). Disarm Safe Mode to proceed.")]
    SafetyLock,

    #[error("Disk is busy (mounted or in use)")]
    DiskBusy,

    #[error("Permission denied")]
    PermissionDenied,

    #[error("Validation failed: {0}")]
    ValidationFailed(String),

    #[allow(dead_code)]
    #[error("Operation aborted by user")]
    Aborted,

    #[allow(dead_code)]
    #[error("Command failed: {0}")]
    CommandFailed(String),
}
