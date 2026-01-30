//! Read-only data sources for TUI (Phase B3)

use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use crate::tui::flash_config::{ImageEditionOption, ImageVersionOption};

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

#[derive(Debug, Clone)]
pub struct ImageMeta {
    pub label: String,
    pub version: String,
    pub edition: String,
    pub path: PathBuf,
    pub is_remote: bool,
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

pub fn collect_local_images(search_paths: &[PathBuf]) -> Vec<ImageMeta> {
    let mut images = Vec::new();
    for dir in search_paths {
        let entries = match fs::read_dir(dir) {
            Ok(entries) => entries,
            Err(_) => continue,
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_file() {
                continue;
            }
            let name = path
                .file_name()
                .map(|name| name.to_string_lossy().to_string())
                .unwrap_or_else(|| "Unnamed image".to_string());
            if !is_image_file(&name) {
                continue;
            }
            let (version, edition) = parse_version_edition(&name);
            images.push(ImageMeta {
                label: format!("{} (local)", name),
                version,
                edition,
                path,
                is_remote: false,
            });
        }
    }

    images
}

pub fn default_image_search_paths() -> Vec<PathBuf> {
    let mut paths = Vec::new();
    if let Ok(value) = env::var("MASH_TUI_IMAGE_DIRS") {
        for part in value.split(':').filter(|part| !part.trim().is_empty()) {
            paths.push(PathBuf::from(part));
        }
    }
    paths.push(PathBuf::from("./images"));
    paths.push(PathBuf::from("."));
    paths.push(PathBuf::from("/opt/mash/images"));
    paths.push(PathBuf::from("/usr/local/share/mash/images"));
    paths.push(PathBuf::from("/var/lib/mash/images"));
    paths.push(PathBuf::from("/tmp"));
    paths
}

pub fn collect_remote_images() -> Vec<ImageMeta> {
    let mut images = Vec::new();
    for version in ImageVersionOption::all() {
        for edition in ImageEditionOption::all() {
            let label = format!(
                "{} {} (remote)",
                version.display(),
                edition.display()
            );
            let filename = format!(
                "fedora-{}-{}-aarch64.raw.xz",
                version.version_str().to_lowercase(),
                edition.edition_str().to_lowercase()
            );
            images.push(ImageMeta {
                label,
                version: version.version_str().to_string(),
                edition: edition.edition_str().to_string(),
                path: PathBuf::from("/tmp").join(filename),
                is_remote: true,
            });
        }
    }

    images
}

fn env_flag(name: &str) -> bool {
    match env::var(name) {
        Ok(value) => matches!(value.to_lowercase().as_str(), "1" | "true" | "yes" | "on"),
        Err(_) => false,
    }
}

fn is_image_file(name: &str) -> bool {
    let name = name.to_lowercase();
    name.ends_with(".raw") || name.ends_with(".img") || name.ends_with(".raw.xz")
}

fn parse_version_edition(name: &str) -> (String, String) {
    let lower = name.to_lowercase();
    let version = if lower.contains("43") {
        "43".to_string()
    } else if lower.contains("42") {
        "42".to_string()
    } else {
        "local".to_string()
    };
    let edition = if lower.contains("kde") {
        "KDE".to_string()
    } else if lower.contains("xfce") {
        "Xfce".to_string()
    } else if lower.contains("lxqt") {
        "LXQt".to_string()
    } else if lower.contains("server") {
        "Server".to_string()
    } else if lower.contains("minimal") {
        "Minimal".to_string()
    } else {
        "Local".to_string()
    };

    (version, edition)
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
