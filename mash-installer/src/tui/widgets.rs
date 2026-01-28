//! Custom widgets for the TUI

use std::process::Command;

/// Information about a disk device
#[derive(Debug, Clone)]
pub struct DiskInfo {
    pub path: String,
    pub name: String,
    pub size: String,
    pub model: String,
    pub is_removable: bool,
}

impl DiskInfo {
    /// Scan the system for available disks
    pub fn scan_disks() -> Vec<DiskInfo> {
        let mut disks = Vec::new();

        // Use lsblk to get disk information
        // -d: only show disks (not partitions)
        // -n: no header
        // -o: output columns
        let output = Command::new("lsblk")
            .args(["-dn", "-o", "NAME,SIZE,MODEL,RM,TYPE"])
            .output();

        if let Ok(output) = output {
            let stdout = String::from_utf8_lossy(&output.stdout);
            for line in stdout.lines() {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 4 {
                    let name = parts[0];
                    let size = parts[1];
                    let dev_type = parts.last().unwrap_or(&"");

                    // Only include disk devices, not partitions or loop devices
                    if *dev_type != "disk" {
                        continue;
                    }

                    // Skip loop devices
                    if name.starts_with("loop") {
                        continue;
                    }

                    // Get model (may be empty or have spaces, so we need to handle carefully)
                    let rm_idx = parts.len() - 2;
                    let is_removable = parts.get(rm_idx).map(|s| *s == "1").unwrap_or(false);

                    // Model is everything between size and RM
                    let model = if parts.len() > 4 {
                        parts[2..rm_idx].join(" ")
                    } else {
                        String::new()
                    };

                    let path = format!("/dev/{}", name);

                    // Skip the root disk
                    if is_root_disk(&path) {
                        continue;
                    }

                    disks.push(DiskInfo {
                        path,
                        name: name.to_string(),
                        size: size.to_string(),
                        model: if model.is_empty() {
                            "Unknown".to_string()
                        } else {
                            model
                        },
                        is_removable,
                    });
                }
            }
        }

        disks
    }

    /// Format for display in list
    pub fn display(&self) -> String {
        let removable_marker = if self.is_removable { " [USB]" } else { "" };
        format!(
            "{} - {} ({}){}",
            self.path, self.size, self.model, removable_marker
        )
    }
}

/// Check if a disk is the root disk (where / is mounted)
fn is_root_disk(disk_path: &str) -> bool {
    let output = Command::new("findmnt")
        .args(["-n", "-o", "SOURCE", "/"])
        .output();

    if let Ok(output) = output {
        let root_dev = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if root_dev.starts_with("/dev/") {
            // Normalize: /dev/sda1 -> /dev/sda, /dev/nvme0n1p2 -> /dev/nvme0n1
            let root_base = base_disk(&root_dev);
            let check_base = base_disk(disk_path);
            return root_base == check_base;
        }
    }
    false
}

/// Get base disk from partition path
fn base_disk(dev: &str) -> String {
    let mut s = dev.to_string();
    // Handle nvme: /dev/nvme0n1p2 -> /dev/nvme0n1
    if s.contains("nvme") {
        if let Some(pos) = s.rfind('p') {
            if s[pos + 1..].chars().all(|c| c.is_ascii_digit()) {
                s.truncate(pos);
            }
        }
        return s;
    }
    // Handle mmcblk: /dev/mmcblk0p2 -> /dev/mmcblk0
    if s.contains("mmcblk") {
        if let Some(pos) = s.rfind('p') {
            if s[pos + 1..].chars().all(|c| c.is_ascii_digit()) {
                s.truncate(pos);
            }
        }
        return s;
    }
    // Handle standard: /dev/sda1 -> /dev/sda
    while s
        .chars()
        .last()
        .map(|c| c.is_ascii_digit())
        .unwrap_or(false)
    {
        s.pop();
    }
    s
}

/// Checkbox state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheckboxState {
    Unchecked,
    Checked,
}

impl CheckboxState {
    pub fn toggle(&mut self) {
        *self = match self {
            CheckboxState::Unchecked => CheckboxState::Checked,
            CheckboxState::Checked => CheckboxState::Unchecked,
        };
    }

    pub fn is_checked(&self) -> bool {
        matches!(self, CheckboxState::Checked)
    }

    pub fn symbol(&self) -> &'static str {
        match self {
            CheckboxState::Unchecked => "[ ]",
            CheckboxState::Checked => "[x]",
        }
    }
}

impl From<bool> for CheckboxState {
    fn from(b: bool) -> Self {
        if b {
            CheckboxState::Checked
        } else {
            CheckboxState::Unchecked
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_base_disk() {
        assert_eq!(base_disk("/dev/sda1"), "/dev/sda");
        assert_eq!(base_disk("/dev/sda"), "/dev/sda");
        assert_eq!(base_disk("/dev/nvme0n1p2"), "/dev/nvme0n1");
        assert_eq!(base_disk("/dev/nvme0n1"), "/dev/nvme0n1");
        assert_eq!(base_disk("/dev/mmcblk0p2"), "/dev/mmcblk0");
    }

    #[test]
    fn test_checkbox() {
        let mut cb = CheckboxState::Unchecked;
        assert!(!cb.is_checked());
        assert_eq!(cb.symbol(), "[ ]");

        cb.toggle();
        assert!(cb.is_checked());
        assert_eq!(cb.symbol(), "[x]");
    }
}
