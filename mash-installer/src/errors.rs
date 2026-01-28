use thiserror::Error;

#[derive(Error, Debug)]
pub enum MashError {
    #[error("Missing --yes-i-know flag. This operation is destructive!")]
    MissingYesIKnow,
    
    #[allow(dead_code)]
    #[error("Operation aborted by user")]
    Aborted,
    
    #[allow(dead_code)]
    #[error("Command failed: {0}")]
    CommandFailed(String),
}
