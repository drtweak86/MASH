use anyhow::{Context, Result};
use mash_hal::procfs::mountinfo as proc_mountinfo;
use mash_hal::MountOps;
use nix::mount::MsFlags;
use std::path::Path;

pub use proc_mountinfo::MountInfo;

pub fn is_mounted(hal: &dyn MountOps, path: &Path) -> Result<bool> {
    Ok(hal.is_mounted(path)?)
}

pub fn mount_device(
    hal: &dyn MountOps,
    device: &Path,
    target: &Path,
    fstype: Option<&str>,
    _flags: MsFlags,
    dry_run: bool,
) -> Result<()> {
    hal.mount_device(
        device,
        target,
        fstype,
        mash_hal::MountOptions::new(),
        dry_run,
    )
    .context("Failed to mount device")?;
    Ok(())
}

pub fn unmount(hal: &dyn MountOps, target: &Path, dry_run: bool) -> Result<()> {
    hal.unmount(target, dry_run).context("Failed to unmount")?;
    Ok(())
}

pub fn parse_mountinfo(content: &str) -> Vec<MountInfo> {
    proc_mountinfo::parse_mountinfo(content)
}

pub fn is_mounted_from_info(path: &Path, entries: &[MountInfo]) -> bool {
    proc_mountinfo::is_mounted_from_info(path, entries)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn parse_mountinfo_extracts_mountpoints() {
        let sample = "36 28 0:31 / / rw,relatime - ext4 /dev/sda3 rw\n".to_string()
            + "37 28 0:32 / /boot rw,relatime - ext4 /dev/sda2 rw\n";
        let entries = parse_mountinfo(&sample);
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].mount_point, PathBuf::from("/"));
        assert_eq!(entries[1].mount_point, PathBuf::from("/boot"));
    }

    #[test]
    fn is_mounted_from_info_matches_paths() {
        let sample = "36 28 0:31 / / rw,relatime - ext4 /dev/sda3 rw\n";
        let entries = parse_mountinfo(sample);
        assert!(is_mounted_from_info(Path::new("/"), &entries));
        assert!(!is_mounted_from_info(Path::new("/mnt"), &entries));
    }

    #[test]
    fn mountinfo_unescapes_paths() {
        let sample = "36 28 0:31 / /mnt/data\\040disk rw,relatime - ext4 /dev/sda3 rw\n";
        let entries = parse_mountinfo(sample);
        assert_eq!(entries[0].mount_point, PathBuf::from("/mnt/data disk"));
    }
}
