use anyhow::{bail, Context, Result};
use std::env;
use std::fs;
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};

/// Resolve the staging root for all download/extract I/O.
///
/// Defaults to `/mnt/mash-staging`. Can be overridden with `MASH_STAGING_ROOT`
/// for tests. Refuses to use the root filesystem unless explicitly allowed via
/// `MASH_ALLOW_ROOT_STAGING=1` (intended only for CI/tests).
pub fn staging_root() -> Result<PathBuf> {
    let default_root = if cfg!(test) {
        "/tmp/mash-staging"
    } else {
        "/mnt/mash-staging"
    };
    let root =
        PathBuf::from(env::var("MASH_STAGING_ROOT").unwrap_or_else(|_| default_root.to_string()));
    fs::create_dir_all(&root)
        .with_context(|| format!("failed to create staging dir at {}", root.display()))?;

    let staging_meta = fs::metadata(&root)?;
    let root_meta = fs::metadata(Path::new("/"))?;
    let allow_root = env::var("MASH_ALLOW_ROOT_STAGING").unwrap_or_default() == "1" || cfg!(test);
    if staging_meta.dev() == root_meta.dev() && !allow_root {
        bail!(
            "Staging directory {} is on the root filesystem; refusing for safety.",
            root.display()
        );
    }

    Ok(root)
}
