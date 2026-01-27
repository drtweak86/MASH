use thiserror::Error;

#[derive(Error, Debug)]
pub enum MashError {
    #[error("Refusing to operate without --yes-i-know (safety latch).")]
    MissingYesIKnow,

    #[error("User aborted.")]
    Aborted,

    #[error("Command failed: {0}")]
    CommandFailed(String),
}
