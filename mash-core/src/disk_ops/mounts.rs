use anyhow::{Context, Result};
use mash_hal::procfs::mountinfo as proc_mountinfo;
use nix::mount::{mount as nix_mount, umount2, MntFlags, MsFlags};
use std::fs;
use std::path::Path;

pub use proc_mountinfo::MountInfo;

pub fn is_mounted(path: &Path) -> Result<bool> {
    let content = fs::read_to_string("/proc/self/mountinfo")
        .context("Failed to read /proc/self/mountinfo")?;
    let entries = parse_mountinfo(&content);
    Ok(is_mounted_from_info(path, &entries))
}

pub fn mount_device(
    device: &Path,
    target: &Path,
    fstype: Option<&str>,
    flags: MsFlags,
    dry_run: bool,
) -> Result<()> {
    if dry_run {
        log::info!(
            "DRY RUN: mount {} -> {}",
            device.display(),
            target.display()
        );
        return Ok(());
    }

    nix_mount(Some(device), target, fstype, flags, None::<&str>)
        .context("Failed to mount device")?;
    Ok(())
}

pub fn unmount(target: &Path, dry_run: bool) -> Result<()> {
    if dry_run {
        log::info!("DRY RUN: unmount {}", target.display());
        return Ok(());
    }

    umount2(target, MntFlags::empty()).context("Failed to unmount")?;
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
