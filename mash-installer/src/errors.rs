use thiserror::Error;

#[derive(Error, Debug)]
pub enum MashError {
    #[error("Missing --yes-i-know flag. This operation is destructive!")]
    MissingYesIKnow,
    
    #[error("Operation aborted by user")]
    Aborted,
    
    #[error("Command failed: {0}")]
    CommandFailed(String),
}
