//! Helpers related to block devices in sysfs.

use anyhow::{anyhow, Result};
use std::fs;
use std::path::Path;

pub fn device_basename(path: &Path) -> Result<String> {
    let name = path
        .file_name()
        .ok_or_else(|| anyhow!("invalid device path {}", path.display()))?
        .to_string_lossy()
        .to_string();
    Ok(name)
}

/// Reads the block device size from `/sys/class/block/<dev>/size`.
///
/// The `size` file is expressed in 512-byte sectors.
pub fn block_device_size_bytes(sys_block_dev_dir: &Path) -> Result<u64> {
    let sectors_str = fs::read_to_string(sys_block_dev_dir.join("size"))?;
    let sectors: u64 = sectors_str.trim().parse()?;
    Ok(sectors.saturating_mul(512))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn block_device_size_bytes_reads_sectors() {
        let tmp = tempdir().unwrap();
        fs::write(tmp.path().join("size"), "8\n").unwrap();
        assert_eq!(block_device_size_bytes(tmp.path()).unwrap(), 4096);
    }

    #[test]
    fn device_basename_extracts_filename() {
        assert_eq!(
            device_basename(Path::new("/dev/sda")).unwrap(),
            "sda".to_string()
        );
    }
}
