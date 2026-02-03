//! Parsing helpers for `/proc/self/mountinfo` (and similar mountinfo files).

use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MountInfo {
    pub mount_point: PathBuf,
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

pub fn mounted_under_device(mountinfo: &str, dev_path: &Path) -> Vec<String> {
    let prefix = dev_path.to_string_lossy().to_string();
    let mut mounts = Vec::new();

    for line in mountinfo.lines() {
        // mountinfo format:
        //   <pre fields...> <mount point> <...> - <fstype> <source> <superopts>
        let (pre, post) = match line.split_once(" - ") {
            Some(v) => v,
            None => continue,
        };
        let mut pre_fields = pre.split_whitespace();
        let _mount_id = pre_fields.next();
        let _parent_id = pre_fields.next();
        let _major_minor = pre_fields.next();
        let _root = pre_fields.next();
        let mount_point = match pre_fields.next() {
            Some(v) => unescape_mount_path(v),
            None => continue,
        };
        let mut post_fields = post.split_whitespace();
        let _fstype = post_fields.next();
        let source = match post_fields.next() {
            Some(v) => v,
            None => continue,
        };
        if source.starts_with(&prefix) {
            mounts.push(mount_point);
        }
    }

    mounts.sort();
    mounts.dedup();
    mounts
}

pub fn root_mount_source(mountinfo: &str) -> Option<String> {
    for line in mountinfo.lines() {
        let (pre, post) = line.split_once(" - ")?;
        let pre_fields: Vec<&str> = pre.split_whitespace().collect();
        if pre_fields.len() < 5 {
            continue;
        }
        let mount_point = unescape_mount_path(pre_fields[4]);
        if mount_point != "/" {
            continue;
        }
        let mut post_fields = post.split_whitespace();
        let _fstype = post_fields.next()?;
        let source = post_fields.next()?.to_string();
        return Some(source);
    }
    None
}

pub fn unescape_mount_path(raw: &str) -> String {
    raw.replace("\\040", " ")
        .replace("\\011", "\t")
        .replace("\\012", "\n")
        .replace("\\134", "\\")
}

fn normalize_path(path: &Path) -> String {
    let s = path.to_string_lossy();
    if s.len() > 1 && s.ends_with('/') {
        s.trim_end_matches('/').to_string()
    } else {
        s.to_string()
    }
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

    #[test]
    fn mounted_under_device_finds_matching_sources() {
        let mi = "36 28 0:31 / / rw,relatime - ext4 /dev/sda3 rw\n\
                  37 28 0:32 / /mnt/boot rw,relatime - ext4 /dev/sda1 rw\n\
                  38 28 0:33 / /mnt/other rw,relatime - ext4 /dev/sdb1 rw\n";
        let mounts = mounted_under_device(mi, Path::new("/dev/sda"));
        assert_eq!(mounts, vec!["/".to_string(), "/mnt/boot".to_string()]);
    }

    #[test]
    fn root_mount_source_extracts_device() {
        let mi = "36 28 0:31 / / rw,relatime - ext4 /dev/sda3 rw\n";
        assert_eq!(root_mount_source(mi), Some("/dev/sda3".to_string()));
    }
}
