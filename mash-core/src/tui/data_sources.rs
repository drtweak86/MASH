//! Read-only data sources for TUI (Phase B3)

use std::collections::HashSet;
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

/// Transport type hint for disk
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransportType {
    Usb,
    Nvme,
    Sata,
    Mmc,
    Scsi,
    Unknown,
}

impl TransportType {
    pub fn hint(&self) -> &'static str {
        match self {
            TransportType::Usb => "USB",
            TransportType::Nvme => "NVMe",
            TransportType::Sata => "SATA",
            TransportType::Mmc => "MMC",
            TransportType::Scsi => "SCSI",
            TransportType::Unknown => "",
        }
    }
}

/// Boot detection confidence level
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BootConfidence {
    Confident,  // Boot device confidently identified
    Unverified, // Could be boot device but uncertain
    NotBoot,    // Definitely not boot device
    Unknown,    // Boot detection failed entirely
}

impl BootConfidence {
    pub fn is_boot(&self) -> bool {
        matches!(self, BootConfidence::Confident | BootConfidence::Unverified)
    }
}

#[derive(Debug, Clone)]
pub struct DiskInfo {
    pub name: String, // Human-readable disk identity (never "sda")
    pub path: String, // /dev/sda
    pub size_bytes: u64,
    pub vendor: Option<String>,
    pub model: Option<String>,
    pub transport: TransportType,
    pub removable: bool,
    pub boot_confidence: BootConfidence,
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

    let (boot_device, boot_detection_succeeded) = boot_device_path_with_confidence();

    for entry in entries.flatten() {
        let dev_name = entry.file_name().to_string_lossy().to_string();
        if should_skip_block_device(&dev_name) {
            continue;
        }

        let sysfs_base = PathBuf::from("/sys/block").join(&dev_name);

        // Read size
        let size_sectors = read_u64(&sysfs_base.join("size")).unwrap_or(0);
        if size_sectors == 0 {
            continue;
        }
        let size_bytes = size_sectors.saturating_mul(512);

        // Read vendor and model
        let vendor = read_trimmed(&sysfs_base.join("device/vendor"));
        let model = read_trimmed(&sysfs_base.join("device/model"));

        // Build human-readable disk identity (never just "sda")
        let identity = build_disk_identity(&dev_name, &vendor, &model, size_bytes);

        // Detect transport type
        let transport = detect_transport_type(&dev_name, &sysfs_base);

        // Check if removable
        let removable = read_u64(&sysfs_base.join("removable")).unwrap_or(0) == 1;

        // Determine boot confidence
        let disk_path = format!("/dev/{}", dev_name);
        let boot_confidence = if !boot_detection_succeeded {
            BootConfidence::Unknown
        } else if boot_device.as_deref() == Some(disk_path.as_str()) {
            BootConfidence::Confident
        } else {
            BootConfidence::NotBoot
        };

        disks.push(DiskInfo {
            name: identity,
            path: disk_path,
            size_bytes,
            vendor,
            model,
            transport,
            removable,
            boot_confidence,
        });
    }

    disks
}

/// Build human-readable disk identity - never returns just device name
fn build_disk_identity(
    _dev_name: &str,
    vendor: &Option<String>,
    model: &Option<String>,
    size_bytes: u64,
) -> String {
    let hw_identity = match (vendor.as_ref(), model.as_ref()) {
        (Some(v), Some(m)) => format!("{} {}", v.trim(), m.trim()),
        (Some(v), None) => v.trim().to_string(),
        (None, Some(m)) => m.trim().to_string(),
        (None, None) => {
            // No vendor/model - use size + generic label
            format!("{} Disk", human_size(size_bytes))
        }
    };

    hw_identity
}

/// Detect transport type from device name and sysfs path
fn detect_transport_type(dev_name: &str, sysfs_base: &Path) -> TransportType {
    // Check device name patterns first
    if dev_name.starts_with("nvme") {
        return TransportType::Nvme;
    }
    if dev_name.starts_with("mmcblk") {
        return TransportType::Mmc;
    }

    // For sd* devices, check sysfs path for transport hints
    if dev_name.starts_with("sd") {
        // Try to read the device path to check for USB
        if let Ok(device_path) = fs::read_link(sysfs_base.join("device")) {
            let path_str = device_path.to_string_lossy().to_lowercase();
            if path_str.contains("usb") {
                return TransportType::Usb;
            }
            if path_str.contains("ata") {
                return TransportType::Sata;
            }
        }
        // Default sd* to SCSI/SATA
        return TransportType::Scsi;
    }

    TransportType::Unknown
}

fn resolve_uuid_to_device_path(uuid: &str) -> Option<String> {
    let by_uuid_path = PathBuf::from("/dev/disk/by-uuid").join(uuid);
    fs::read_link(&by_uuid_path).ok().and_then(|path| {
        // Canonicalize to get the /dev/sdX path
        let canonical = PathBuf::from("/dev/disk/by-uuid")
            .join(&path)
            .canonicalize()
            .ok();
        canonical.map(|p| p.to_string_lossy().to_string())
    })
}

fn get_boot_device_from_cmdline() -> Option<String> {
    let cmdline = fs::read_to_string("/proc/cmdline").ok()?;
    for part in cmdline.split_whitespace() {
        if part.starts_with("root=") {
            let root_val = part.trim_start_matches("root=");
            if root_val.starts_with("UUID=") {
                let uuid = root_val.trim_start_matches("UUID=");
                return resolve_uuid_to_device_path(uuid);
            } else {
                return Some(root_val.to_string());
            }
        }
    }
    None
}

pub fn boot_device_path() -> Option<String> {
    boot_device_path_with_confidence().0
}

/// Returns (boot_device_path, detection_succeeded)
/// detection_succeeded is false if we couldn't confidently identify the boot device
fn boot_device_path_with_confidence() -> (Option<String>, bool) {
    // Prioritize /proc/cmdline
    if let Some(cmdline_device) = get_boot_device_from_cmdline() {
        if let Some(device) = base_block_device(&cmdline_device) {
            return (Some(device), true);
        }
    }

    // Fallback to /proc/self/mounts
    if let Ok(mounts) = fs::read_to_string("/proc/self/mounts") {
        for line in mounts.lines() {
            let mut parts = line.split_whitespace();
            if let (Some(device), Some(mountpoint)) = (parts.next(), parts.next()) {
                if mountpoint == "/" {
                    if let Some(device) = base_block_device(device) {
                        return (Some(device), true);
                    }
                }
            }
        }
    }

    // Boot detection failed
    (None, false)
}

fn base_block_device(device: &str) -> Option<String> {
    if !device.starts_with("/dev/") {
        return None;
    }
    let name = device.trim_start_matches("/dev/");
    let base = if name.starts_with("nvme") || name.starts_with("mmcblk") || name.starts_with("loop")
    {
        if let Some(idx) = name.rfind('p') {
            let suffix = &name[idx + 1..];
            if !suffix.is_empty() && suffix.chars().all(|c| c.is_ascii_digit()) {
                name[..idx].to_string()
            } else {
                name.to_string()
            }
        } else {
            name.to_string()
        }
    } else {
        let trimmed = name.trim_end_matches(|c: char| c.is_ascii_digit());
        if trimmed.is_empty() {
            name.to_string()
        } else {
            trimmed.to_string()
        }
    };
    Some(format!("/dev/{}", base))
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
            let label = format!("{} {} (remote)", version.display(), edition.display());
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

pub fn collect_locales() -> Vec<String> {
    let locales = load_supported_locales();
    if locales.is_empty() {
        return Vec::new();
    }
    let layouts = load_xkb_layouts();
    locales
        .into_iter()
        .map(|locale| {
            let keymap = derive_keymap(&locale, &layouts);
            format!("{}:{}", locale, keymap)
        })
        .collect()
}

fn env_flag(name: &str) -> bool {
    match env::var(name) {
        Ok(value) => matches!(value.to_lowercase().as_str(), "1" | "true" | "yes" | "on"),
        Err(_) => false,
    }
}

fn load_supported_locales() -> Vec<String> {
    let content = fs::read_to_string("/usr/share/i18n/SUPPORTED").unwrap_or_default();
    let mut locales = Vec::new();
    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let locale = line.split_whitespace().next().unwrap_or("");
        if locale.is_empty() || !locale.contains("UTF-8") {
            continue;
        }
        locales.push(locale.to_string());
    }
    locales
}

fn load_xkb_layouts() -> HashSet<String> {
    let content = fs::read_to_string("/usr/share/X11/xkb/rules/base.lst").unwrap_or_default();
    let mut layouts = HashSet::new();
    let mut in_layouts = false;
    for line in content.lines() {
        let line = line.trim_end();
        if line.starts_with('!') {
            in_layouts = line.contains("layout");
            continue;
        }
        if !in_layouts {
            continue;
        }
        let mut parts = line.split_whitespace();
        if let Some(code) = parts.next() {
            layouts.insert(code.to_lowercase());
        }
    }
    layouts
}

fn derive_keymap(locale: &str, layouts: &HashSet<String>) -> String {
    let base = locale
        .split('.')
        .next()
        .unwrap_or(locale)
        .split('@')
        .next()
        .unwrap_or(locale);
    let mut parts = base.split('_');
    let lang = parts.next().unwrap_or("en").to_lowercase();
    let country = parts.next().unwrap_or("").to_uppercase();
    let country_layout = normalize_country_layout(&country);
    if !country_layout.is_empty() && layouts.contains(&country_layout) {
        return country_layout;
    }
    if layouts.contains(&lang) {
        return lang;
    }
    if layouts.contains("us") {
        return "us".to_string();
    }
    "us".to_string()
}

fn normalize_country_layout(country: &str) -> String {
    match country {
        "GB" => "gb".to_string(),
        "US" => "us".to_string(),
        "DE" => "de".to_string(),
        "FR" => "fr".to_string(),
        "ES" => "es".to_string(),
        "IT" => "it".to_string(),
        "NL" => "nl".to_string(),
        "PT" => "pt".to_string(),
        "SE" => "se".to_string(),
        "NO" => "no".to_string(),
        "DK" => "dk".to_string(),
        _ => country.to_lowercase(),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base_block_device_strips_partition_from_sd() {
        assert_eq!(base_block_device("/dev/sda1"), Some("/dev/sda".to_string()));
        assert_eq!(base_block_device("/dev/sda5"), Some("/dev/sda".to_string()));
        assert_eq!(
            base_block_device("/dev/sdb12"),
            Some("/dev/sdb".to_string())
        );
    }

    #[test]
    fn base_block_device_strips_partition_from_nvme() {
        assert_eq!(
            base_block_device("/dev/nvme0n1p1"),
            Some("/dev/nvme0n1".to_string())
        );
        assert_eq!(
            base_block_device("/dev/nvme0n1p5"),
            Some("/dev/nvme0n1".to_string())
        );
        assert_eq!(
            base_block_device("/dev/nvme1n1p2"),
            Some("/dev/nvme1n1".to_string())
        );
    }

    #[test]
    fn base_block_device_strips_partition_from_mmcblk() {
        assert_eq!(
            base_block_device("/dev/mmcblk0p1"),
            Some("/dev/mmcblk0".to_string())
        );
        assert_eq!(
            base_block_device("/dev/mmcblk0p2"),
            Some("/dev/mmcblk0".to_string())
        );
        assert_eq!(
            base_block_device("/dev/mmcblk1p3"),
            Some("/dev/mmcblk1".to_string())
        );
    }

    #[test]
    fn base_block_device_handles_whole_disks() {
        assert_eq!(base_block_device("/dev/sda"), Some("/dev/sda".to_string()));
        assert_eq!(
            base_block_device("/dev/nvme0n1"),
            Some("/dev/nvme0n1".to_string())
        );
        assert_eq!(
            base_block_device("/dev/mmcblk0"),
            Some("/dev/mmcblk0".to_string())
        );
    }

    #[test]
    fn base_block_device_rejects_non_dev_paths() {
        assert_eq!(base_block_device("/sys/block/sda"), None);
        assert_eq!(base_block_device("sda1"), None);
        assert_eq!(base_block_device(""), None);
    }

    #[test]
    fn should_skip_loop_devices() {
        assert!(should_skip_block_device("loop0"));
        assert!(should_skip_block_device("loop1"));
    }

    #[test]
    fn should_skip_ram_devices() {
        assert!(should_skip_block_device("ram0"));
        assert!(should_skip_block_device("ram1"));
    }

    #[test]
    fn should_skip_optical_drives() {
        assert!(should_skip_block_device("sr0"));
        assert!(should_skip_block_device("sr1"));
    }

    #[test]
    fn should_skip_device_mapper() {
        assert!(should_skip_block_device("dm-0"));
        assert!(should_skip_block_device("dm-1"));
    }

    #[test]
    fn should_not_skip_valid_disks() {
        assert!(!should_skip_block_device("sda"));
        assert!(!should_skip_block_device("sdb"));
        assert!(!should_skip_block_device("nvme0n1"));
        assert!(!should_skip_block_device("mmcblk0"));
    }

    #[test]
    fn human_size_formats_bytes() {
        assert_eq!(human_size(512), "512 B");
        assert_eq!(human_size(1023), "1023 B");
    }

    #[test]
    fn human_size_formats_kb() {
        assert_eq!(human_size(1024), "1.0 KiB");
        assert_eq!(human_size(2048), "2.0 KiB");
    }

    #[test]
    fn human_size_formats_mb() {
        assert_eq!(human_size(1024 * 1024), "1.0 MiB");
        assert_eq!(human_size(512 * 1024 * 1024), "512.0 MiB");
    }

    #[test]
    fn human_size_formats_gb() {
        assert_eq!(human_size(1024 * 1024 * 1024), "1.0 GiB");
        assert_eq!(human_size(32 * 1024 * 1024 * 1024), "32.0 GiB");
    }
}
