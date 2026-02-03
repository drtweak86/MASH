use thiserror::Error;

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
}
