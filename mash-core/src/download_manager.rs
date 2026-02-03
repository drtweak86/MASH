use crate::download;
use anyhow::Result;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;

/// A small helper wrapper around `crate::download` that:
/// - standardizes where artifacts are stored (under a caller-provided root)
/// - centralizes cleanup on error/cancel
///
/// This module intentionally has no dependency on the TUI.
pub fn fetch_fedora_image(
    download_root: &Path,
    version: &str,
    edition: &str,
    progress: &mut dyn FnMut(download::DownloadProgress) -> bool,
    stage: &mut dyn FnMut(&str),
    cancel_flag: Option<&AtomicBool>,
) -> Result<PathBuf> {
    let images_dir = download_root.join("images");
    match download::download_fedora_image_with_progress(
        &images_dir,
        version,
        edition,
        progress,
        stage,
        cancel_flag,
    ) {
        Ok(path) => Ok(path),
        Err(err) => {
            cleanup_fedora_artifacts(&images_dir, version, edition);
            Err(err)
        }
    }
}

pub fn fetch_uefi_bundle(
    download_root: &Path,
    progress: &mut dyn FnMut(download::DownloadProgress) -> bool,
    stage: &mut dyn FnMut(&str),
    cancel_flag: Option<&AtomicBool>,
) -> Result<PathBuf> {
    let uefi_dir = download_root.join("uefi");
    match download::download_uefi_firmware_with_progress(&uefi_dir, progress, stage, cancel_flag) {
        Ok(path) => Ok(path),
        Err(err) => {
            cleanup_uefi_artifacts(&uefi_dir);
            Err(err)
        }
    }
}

pub(crate) fn cleanup_fedora_artifacts(base: &Path, version: &str, edition: &str) {
    let arch = "aarch64";
    let raw_name = format!("Fedora-{}-{}-{}.raw", edition, version, arch);
    let xz_name = format!("Fedora-{}-{}-{}.raw.xz", edition, version, arch);
    let _ = std::fs::remove_file(base.join(raw_name));
    let _ = std::fs::remove_file(base.join(xz_name));
}

pub(crate) fn cleanup_uefi_artifacts(base: &Path) {
    let _ = std::fs::remove_dir_all(base);
}
