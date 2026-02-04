//! Helpers related to block devices in sysfs.

use anyhow::{anyhow, Result};
use std::fs;
use std::path::{Path, PathBuf};

/// Best-effort transport type hint for a block device.
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
            TransportType::Unknown => "Unknown",
        }
    }
}

/// How the UI should tag the boot relationship for a disk.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BootTag {
    BootMedia,
    BootMaybe,
    NotBoot,
    Unknown,
}

impl BootTag {
    fn label(&self) -> &'static str {
        match self {
            BootTag::BootMedia => "BOOT MEDIA",
            BootTag::BootMaybe => "BOOT?",
            BootTag::NotBoot => "BOOT: NO",
            BootTag::Unknown => "BOOT: UNKNOWN",
        }
    }
}

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

/// Best-effort transport hints from sysfs.
///
/// This reads the `device` symlink under a sysfs block device directory (e.g. `/sys/block/sda`).
pub fn transport_path_hint(sys_block_dev_dir: &Path) -> Option<String> {
    fs::read_link(sys_block_dev_dir.join("device"))
        .ok()
        .map(|p| p.to_string_lossy().to_lowercase())
}

/// Best-effort transport detection from device name and sysfs path.
pub fn detect_transport_type(dev_name: &str, sysfs_base: &Path) -> TransportType {
    // Device name patterns first.
    if dev_name.starts_with("nvme") {
        return TransportType::Nvme;
    }
    if dev_name.starts_with("mmcblk") {
        return TransportType::Mmc;
    }

    // For sd* devices, consult sysfs for USB/ATA hints.
    if dev_name.starts_with("sd") {
        if let Some(path_str) = transport_path_hint(sysfs_base) {
            if path_str.contains("usb") {
                return TransportType::Usb;
            }
            if path_str.contains("ata") {
                return TransportType::Sata;
            }
        }
        // sd* is often SCSI (including SATA behind a SCSI layer).
        return TransportType::Scsi;
    }

    TransportType::Unknown
}

/// Canonical, non-device-first disk label for UI display.
///
/// Requirements:
/// - Never "device-first" (do not start with `/dev/sdX`).
/// - Always include: vendor+model (or explicit missing-sysfs warning), size, transport,
///   removable/internal, and a boot tag.
pub fn canonical_disk_label(dev: &BlockDeviceInfo, boot: BootTag) -> String {
    let size = human_size(dev.size_bytes);
    let transport = detect_transport_type(&dev.name, &dev.sysfs_path).hint();
    let location = if dev.removable {
        "REMOVABLE"
    } else {
        "INTERNAL"
    };

    let vendor = dev
        .vendor
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty());
    let model = dev
        .model
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty());
    let identity = match (vendor, model) {
        (Some(v), Some(m)) => format!("{} {}", v, m),
        _ => format!("Unknown disk ({}) - missing sysfs data", size),
    };

    format!(
        "{} ({}) {} [{}] [{}]",
        identity,
        transport,
        size,
        location,
        boot.label()
    )
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

fn human_size(bytes: u64) -> String {
    const KIB: f64 = 1024.0;
    const MIB: f64 = KIB * 1024.0;
    const GIB: f64 = MIB * 1024.0;
    let value = bytes as f64;

    if value >= GIB {
        format!("{:.1} GiB", value / GIB)
    } else if value >= MIB {
        format!("{:.1} MiB", value / MIB)
    } else if value >= KIB {
        format!("{:.1} KiB", value / KIB)
    } else {
        format!("{} B", bytes)
    }
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

    #[test]
    fn canonical_label_includes_required_parts_and_is_not_device_first() {
        let dev = BlockDeviceInfo {
            name: "nvme0n1".to_string(),
            dev_path: PathBuf::from("/dev/nvme0n1"),
            sysfs_path: PathBuf::from("/sys/block/nvme0n1"),
            size_bytes: 1024 * 1024 * 1024,
            vendor: Some("ACME".to_string()),
            model: Some("FastDisk".to_string()),
            serial: None,
            wwn: None,
            removable: false,
        };
        let label = canonical_disk_label(&dev, BootTag::NotBoot);
        assert!(!label.starts_with("/dev/"));
        assert!(label.contains("ACME FastDisk"));
        assert!(label.contains("GiB"));
        assert!(label.contains("NVMe"));
        assert!(label.contains("INTERNAL"));
        assert!(label.contains("BOOT: NO"));
    }

    #[test]
    fn canonical_label_warns_when_sysfs_identity_missing() {
        let dev = BlockDeviceInfo {
            name: "sda".to_string(),
            dev_path: PathBuf::from("/dev/sda"),
            sysfs_path: PathBuf::from("/sys/block/sda"),
            size_bytes: 1024,
            vendor: None,
            model: None,
            serial: None,
            wwn: None,
            removable: true,
        };
        let label = canonical_disk_label(&dev, BootTag::Unknown);
        assert!(label.contains("Unknown disk"));
        assert!(label.contains("missing sysfs data"));
        assert!(label.contains("REMOVABLE"));
        assert!(label.contains("BOOT: UNKNOWN"));
    }
}
