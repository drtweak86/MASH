use thiserror::Error;

pub type Result<T> = anyhow::Result<T>;

#[derive(Debug, Error)]
pub enum MashError {
    #[error("Invalid disk path: {0}")]
    InvalidDisk(String),

    #[error("Refusing to operate on system/root disk: {0}")]
    RefuseRootDisk(String),

    #[error("Command failed: {cmd}\n{stderr}")]
    CommandFailed { cmd: String, stderr: String },

    #[error("{0}")]
    Other(String),
}
