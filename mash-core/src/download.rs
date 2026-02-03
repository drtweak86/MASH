//! Deprecated legacy download entry points.
//!
//! WO-036.1: MASH now uses `crate::downloader` as the single source of truth for all downloads
//! (indexed, checksum verified, resumable), with `crate::download_manager` for extraction helpers.
//!
//! This module remains as a compatibility shim only.

use anyhow::Result;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;

/// Progress type for downloads (re-exported from the unified downloader).
pub use crate::downloader::DownloadProgress;

#[deprecated(note = "Use mash_core::download_manager::fetch_fedora_image or mash_core::downloader")]
pub fn download_fedora_image(
    destination_dir: &Path,
    version: &str,
    edition: &str,
) -> Result<PathBuf> {
    let root = destination_dir.parent().unwrap_or_else(|| Path::new("."));
    let mut progress = |_p: DownloadProgress| true;
    let mut stage = |_msg: &str| {};
    crate::download_manager::fetch_fedora_image(
        root,
        version,
        edition,
        &mut progress,
        &mut stage,
        None,
    )
}

#[deprecated(note = "Use mash_core::download_manager::fetch_fedora_image or mash_core::downloader")]
pub fn download_fedora_image_with_progress(
    destination_dir: &Path,
    version: &str,
    edition: &str,
    progress: &mut dyn FnMut(DownloadProgress) -> bool,
    stage: &mut dyn FnMut(&str),
    cancel_flag: Option<&AtomicBool>,
) -> Result<PathBuf> {
    let root = destination_dir.parent().unwrap_or_else(|| Path::new("."));
    crate::download_manager::fetch_fedora_image(
        root,
        version,
        edition,
        progress,
        stage,
        cancel_flag,
    )
}

#[deprecated(note = "Use mash_core::download_manager::fetch_uefi_bundle or mash_core::downloader")]
pub fn download_uefi_firmware(destination_dir: &Path) -> Result<()> {
    let root = destination_dir.parent().unwrap_or_else(|| Path::new("."));
    let mut progress = |_p: DownloadProgress| true;
    let mut stage = |_msg: &str| {};
    let _ = crate::download_manager::fetch_uefi_bundle(root, &mut progress, &mut stage, None)?;
    Ok(())
}

#[deprecated(note = "Use mash_core::download_manager::fetch_uefi_bundle or mash_core::downloader")]
pub fn download_uefi_firmware_with_progress(
    destination_dir: &Path,
    progress: &mut dyn FnMut(DownloadProgress) -> bool,
    stage: &mut dyn FnMut(&str),
    cancel_flag: Option<&AtomicBool>,
) -> Result<PathBuf> {
    let root = destination_dir.parent().unwrap_or_else(|| Path::new("."));
    crate::download_manager::fetch_uefi_bundle(root, progress, stage, cancel_flag)
}
