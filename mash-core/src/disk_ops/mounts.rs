use anyhow::{Context, Result};
use nix::mount::{mount as nix_mount, umount2, MntFlags, MsFlags};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MountInfo {
    pub mount_point: PathBuf,
}

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
    content
        .lines()
        .filter_map(|line| {
            let mut parts = line.split(" - ");
            let pre = parts.next()?;
            let pre_fields: Vec<&str> = pre.split_whitespace().collect();
            if pre_fields.len() < 5 {
                return None;
            }
            let mount_point = unescape_mount_path(pre_fields[4]);
            Some(MountInfo {
                mount_point: PathBuf::from(mount_point),
            })
        })
        .collect()
}

pub fn is_mounted_from_info(path: &Path, entries: &[MountInfo]) -> bool {
    let target = normalize_path(path);
    entries
        .iter()
        .any(|entry| normalize_path(&entry.mount_point) == target)
}

fn normalize_path(path: &Path) -> String {
    let s = path.to_string_lossy();
    if s.len() > 1 && s.ends_with('/') {
        s.trim_end_matches('/').to_string()
    } else {
        s.to_string()
    }
}

fn unescape_mount_path(raw: &str) -> String {
    raw.replace("\\040", " ")
        .replace("\\011", "\t")
        .replace("\\012", "\n")
        .replace("\\134", "\\")
}

#[cfg(test)]
mod tests {
    use super::*;

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
