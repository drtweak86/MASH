use thiserror::Error;

#[derive(Error, Debug)]
pub enum MashError {
    #[error("Missing --yes-i-know flag. This operation is destructive!")]
    MissingYesIKnow,
    
    #[error("Operation aborted by user")]
    Aborted,
    
    #[error("Command failed: {0}")]
    CommandFailed(String),
    
    #[error("Image file not found or invalid: {0}")]
    InvalidImage(String),
    
    #[error("Disk not found: {0}")]
    DiskNotFound(String),
    
    #[error("Insufficient disk space. Need at least 3.7TB")]
    InsufficientSpace,
    
    #[error("Partition error: {0}")]
    PartitionError(String),
    
    #[error("Mount error: {0}")]
    MountError(String),
    
    #[error("UEFI configuration failed: {0}")]
    UefiError(String),
}
