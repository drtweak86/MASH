//! Helpers related to block devices in sysfs.

use anyhow::{anyhow, Result};
use std::fs;
use std::path::{Path, PathBuf};

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

#[derive(Debug, Clone)]
pub struct BlockDeviceInfo {
    pub name: String,
    pub dev_path: PathBuf,
    pub sysfs_path: PathBuf,
    pub size_bytes: u64,
    pub vendor: Option<String>,
    pub model: Option<String>,
    pub serial: Option<String>,
    pub wwn: Option<String>,
    pub removable: bool,
}

pub fn scan_block_devices() -> Result<Vec<BlockDeviceInfo>> {
    scan_block_devices_in(Path::new("/sys/block"))
}

pub fn scan_block_devices_in(sys_block_root: &Path) -> Result<Vec<BlockDeviceInfo>> {
    let mut out = Vec::new();
    let entries = fs::read_dir(sys_block_root)?;
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        if should_skip_block_device(&name) {
            continue;
        }
        let sysfs_path = sys_block_root.join(&name);
        let size_bytes = match block_device_size_bytes(&sysfs_path) {
            Ok(v) => v,
            Err(_) => continue,
        };
        if size_bytes == 0 {
            continue;
        }

        let dev_path = PathBuf::from("/dev").join(&name);
        let vendor = read_trimmed(sysfs_path.join("device/vendor"));
        let model = read_trimmed(sysfs_path.join("device/model"));
        let serial = read_trimmed(sysfs_path.join("device/serial"));
        let wwn = read_trimmed(sysfs_path.join("device/wwid"))
            .or_else(|| read_trimmed(sysfs_path.join("device/wwn")));
        let removable = read_trimmed(sysfs_path.join("removable"))
            .and_then(|v| v.parse::<u64>().ok())
            .unwrap_or(0)
            == 1;

        out.push(BlockDeviceInfo {
            name,
            dev_path,
            sysfs_path,
            size_bytes,
            vendor,
            model,
            serial,
            wwn,
            removable,
        });
    }
    Ok(out)
}

fn read_trimmed(path: PathBuf) -> Option<String> {
    fs::read_to_string(path)
        .ok()
        .map(|s| {
            let t = s.trim().to_string();
            if t.is_empty() {
                String::new()
            } else {
                t
            }
        })
        .and_then(|s| if s.is_empty() { None } else { Some(s) })
}

fn should_skip_block_device(name: &str) -> bool {
    // Avoid ephemeral / virtual devices in TUI disk selection.
    name.starts_with("loop")
        || name.starts_with("ram")
        || name.starts_with("zram")
        || name.starts_with("dm-")
        || name.starts_with("md")
        || name.starts_with("sr")
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

    #[test]
    fn scan_block_devices_in_skips_virtual_and_reads_size() {
        let tmp = tempdir().unwrap();
        fs::create_dir_all(tmp.path().join("sda")).unwrap();
        fs::write(tmp.path().join("sda/size"), "8\n").unwrap();
        fs::create_dir_all(tmp.path().join("loop0")).unwrap();
        fs::write(tmp.path().join("loop0/size"), "8\n").unwrap();

        let disks = scan_block_devices_in(tmp.path()).unwrap();
        assert_eq!(disks.len(), 1);
        assert_eq!(disks[0].name, "sda");
        assert_eq!(disks[0].size_bytes, 4096);
    }
}
