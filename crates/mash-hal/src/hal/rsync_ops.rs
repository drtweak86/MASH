//! High-level file copy operations.

use anyhow::Result;
use std::path::Path;

#[derive(Debug, Clone, Default)]
pub struct RsyncOptions {
    pub archive: bool,
    pub preserve_xattrs: bool,
    pub preserve_acls: bool,
    pub numeric_ids: bool,
    /// rsync `--info` value (e.g. `progress2`).
    pub info: Option<String>,
    /// Extra rsync args (verbatim).
    pub extra_args: Vec<String>,
}

impl RsyncOptions {
    pub fn progress2_archive() -> Self {
        Self {
            archive: true,
            preserve_xattrs: true,
            preserve_acls: true,
            numeric_ids: true,
            info: Some("progress2".to_string()),
            extra_args: Vec::new(),
        }
    }

    pub fn vfat_safe() -> Self {
        Self {
            archive: false,
            preserve_xattrs: false,
            preserve_acls: false,
            numeric_ids: false,
            info: None,
            extra_args: vec![
                "-rltD".to_string(),
                "--no-owner".to_string(),
                "--no-group".to_string(),
                "--no-perms".to_string(),
            ],
        }
    }
}

pub trait RsyncOps {
    /// Run rsync, streaming stdout line-by-line into `on_stdout_line`.
    ///
    /// Return an error if rsync fails. If `on_stdout_line` returns false, rsync should be aborted
    /// and the call should return an error.
    fn rsync_stream_stdout(
        &self,
        src: &Path,
        dst: &Path,
        opts: &RsyncOptions,
        on_stdout_line: &mut dyn FnMut(&str) -> bool,
    ) -> Result<()>;
}
