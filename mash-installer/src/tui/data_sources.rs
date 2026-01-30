//! Read-only data sources for TUI (Phase B3)

use std::env;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy)]
pub struct DataFlags {
    pub disks: bool,
    pub images: bool,
    pub locales: bool,
}

#[derive(Debug, Clone)]
pub struct DiskInfo {
    pub name: String,
    pub path: String,
    pub size_bytes: u64,
    pub model: Option<String>,
    pub removable: bool,
}

pub fn data_flags() -> DataFlags {
    let global = env_flag("MASH_TUI_REAL_DATA");
    DataFlags {
        disks: env_flag("MASH_TUI_REAL_DISKS") || global,
        images: env_flag("MASH_TUI_REAL_IMAGES") || global,
        locales: env_flag("MASH_TUI_REAL_LOCALES") || global,
    }
}

pub fn scan_disks() -> Vec<DiskInfo> {
    let mut disks = Vec::new();
    let entries = match fs::read_dir("/sys/block") {
        Ok(entries) => entries,
        Err(_) => return disks,
    };

    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        if should_skip_block_device(&name) {
            continue;
        }
        let size_path = PathBuf::from("/sys/block").join(&name).join("size");
        let size_sectors = read_u64(&size_path).unwrap_or(0);
        if size_sectors == 0 {
            continue;
        }
        let size_bytes = size_sectors.saturating_mul(512);
        let model = read_trimmed(
            &PathBuf::from("/sys/block")
                .join(&name)
                .join("device/model"),
        );
        let vendor = read_trimmed(
            &PathBuf::from("/sys/block")
                .join(&name)
                .join("device/vendor"),
        );
        let label = match (vendor, model) {
            (Some(vendor), Some(model)) => Some(format!("{} {}", vendor, model).trim().to_string()),
            (Some(vendor), None) => Some(vendor),
            (None, Some(model)) => Some(model),
            _ => None,
        };
        let removable_path = PathBuf::from("/sys/block")
            .join(&name)
            .join("removable");
        let removable = read_u64(&removable_path).unwrap_or(0) == 1;

        disks.push(DiskInfo {
            name: label.unwrap_or_else(|| name.clone()),
            path: format!("/dev/{}", name),
            size_bytes,
            removable,
        });
    }

    disks
}

pub fn human_size(bytes: u64) -> String {
    const KB: f64 = 1024.0;
    const MB: f64 = KB * 1024.0;
    const GB: f64 = MB * 1024.0;
    let value = bytes as f64;
    if value >= GB {
        format!("{:.1} GiB", value / GB)
    } else if value >= MB {
        format!("{:.1} MiB", value / MB)
    } else if value >= KB {
        format!("{:.1} KiB", value / KB)
    } else {
        format!("{} B", bytes)
    }
}

fn env_flag(name: &str) -> bool {
    match env::var(name) {
        Ok(value) => matches!(value.to_lowercase().as_str(), "1" | "true" | "yes" | "on"),
        Err(_) => false,
    }
}

fn should_skip_block_device(name: &str) -> bool {
    name.starts_with("loop")
        || name.starts_with("ram")
        || name.starts_with("sr")
        || name.starts_with("fd")
        || name.starts_with("zram")
        || name.starts_with("dm-")
}

fn read_trimmed(path: &Path) -> Option<String> {
    fs::read_to_string(path)
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

fn read_u64(path: &Path) -> Option<u64> {
    read_trimmed(path).and_then(|value| value.parse::<u64>().ok())
}
