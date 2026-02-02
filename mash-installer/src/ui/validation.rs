//! Input validation guard rails for user-provided paths.

use std::path::Path;

pub fn validate_disk_path(disk: &str) -> Result<(), String> {
    let trimmed = disk.trim();
    if trimmed.is_empty() {
        return Err("Disk path is required.".to_string());
    }
    if !trimmed.starts_with("/dev/") {
        return Err("Disk device must start with /dev/.".to_string());
    }
    if trimmed.chars().any(|c| c.is_whitespace()) {
        return Err("Disk device path must not contain whitespace.".to_string());
    }
    if !Path::new(trimmed).exists() {
        return Err(format!("Disk device not found: {}", trimmed));
    }
    Ok(())
}

pub fn validate_image_path(path: &Path) -> Result<(), String> {
    if path.as_os_str().is_empty() {
        return Err("Image path is required.".to_string());
    }
    if !path.exists() {
        return Err(format!("Image file not found: {}", path.display()));
    }
    if !path.is_file() {
        return Err(format!("Image path is not a file: {}", path.display()));
    }
    Ok(())
}

pub fn validate_uefi_dir(path: &Path) -> Result<(), String> {
    if path.as_os_str().is_empty() {
        return Err("UEFI directory is required.".to_string());
    }
    if !path.exists() || !path.is_dir() {
        return Err(format!("UEFI directory not found: {}", path.display()));
    }
    let rpi_efi = path.join("RPI_EFI.fd");
    if !rpi_efi.exists() {
        return Err(format!("Missing required UEFI file: {}", rpi_efi.display()));
    }
    Ok(())
}
