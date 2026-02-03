use crate::downloader::{AssetKey, DownloadOptions, DownloadProgress, ImageKey, OsKind};
use anyhow::{Context, Result};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use zip::ZipArchive;

/// A small helper wrapper around `crate::downloader` that:
/// - standardizes where artifacts are stored (under a caller-provided root)
/// - centralizes cleanup on error/cancel
///
/// This module intentionally has no dependency on the TUI.
pub fn fetch_fedora_image(
    download_root: &Path,
    _version: &str,
    _edition: &str,
    progress: &mut dyn FnMut(DownloadProgress) -> bool,
    stage: &mut dyn FnMut(&str),
    cancel_flag: Option<&AtomicBool>,
) -> Result<PathBuf> {
    let images_dir = download_root.join("images");

    // Fedora full-loop currently expects the canonical KDE Mobile Disk image entry.
    // The index provides checksum verification and resume.
    stage("Downloading Fedora image (checksum verified)...");
    let mut cb = |p: DownloadProgress| {
        if cancel_flag
            .map(|flag| flag.load(Ordering::Relaxed))
            .unwrap_or(false)
        {
            return false;
        }
        progress(p)
    };
    let opts = DownloadOptions {
        image: Some(ImageKey {
            os: OsKind::Fedora,
            variant: "kde_mobile_disk".to_string(),
            arch: "aarch64".to_string(),
        }),
        download_dir: images_dir.clone(),
        ..Default::default()
    };

    let result = crate::downloader::download_with_progress(&opts, &mut cb)
        .map(|artifact| artifact.path)
        .context("failed to download Fedora image");

    match result {
        Ok(path) => Ok(path),
        Err(err) => {
            cleanup_fedora_artifacts(&images_dir);
            Err(err)
        }
    }
}

pub fn fetch_uefi_bundle(
    download_root: &Path,
    progress: &mut dyn FnMut(DownloadProgress) -> bool,
    stage: &mut dyn FnMut(&str),
    cancel_flag: Option<&AtomicBool>,
) -> Result<PathBuf> {
    let uefi_dir = download_root.join("uefi");
    fs::create_dir_all(&uefi_dir).context("failed to create uefi download dir")?;

    stage("Downloading UEFI bundle (checksum verified)...");
    let mut cb = |p: DownloadProgress| {
        if cancel_flag
            .map(|flag| flag.load(Ordering::Relaxed))
            .unwrap_or(false)
        {
            return false;
        }
        progress(p)
    };

    let zip_dir = uefi_dir.clone();
    let opts = DownloadOptions {
        asset: Some(AssetKey {
            name: "rpi4_uefi_firmware_zip_v1_50".to_string(),
        }),
        download_dir: zip_dir.clone(),
        ..Default::default()
    };
    let zip_path = match crate::downloader::download_with_progress(&opts, &mut cb) {
        Ok(artifact) => artifact.path,
        Err(err) => {
            cleanup_uefi_artifacts(&uefi_dir);
            return Err(err);
        }
    };

    if cancel_flag
        .map(|flag| flag.load(Ordering::Relaxed))
        .unwrap_or(false)
    {
        cleanup_uefi_artifacts(&uefi_dir);
        anyhow::bail!("cancelled");
    }

    stage("Extracting UEFI bundle...");
    let file = fs::File::open(&zip_path).context("failed to open downloaded uefi zip")?;
    let mut archive = ZipArchive::new(file).context("failed to read uefi zip")?;
    for i in 0..archive.len() {
        if cancel_flag
            .map(|flag| flag.load(Ordering::Relaxed))
            .unwrap_or(false)
        {
            cleanup_uefi_artifacts(&uefi_dir);
            anyhow::bail!("cancelled");
        }
        let mut entry = archive.by_index(i)?;
        let outpath = match entry.enclosed_name() {
            Some(path) => uefi_dir.join(path),
            None => continue,
        };
        if entry.name().ends_with('/') {
            fs::create_dir_all(&outpath)?;
            continue;
        }
        if let Some(parent) = outpath.parent() {
            fs::create_dir_all(parent)?;
        }
        let mut out = fs::File::create(&outpath)?;
        std::io::copy(&mut entry, &mut out)?;
    }
    let _ = fs::remove_file(&zip_path);

    // The flash pipeline expects this file to exist.
    let rpi_efi = uefi_dir.join("RPI_EFI.fd");
    if !rpi_efi.exists() {
        cleanup_uefi_artifacts(&uefi_dir);
        anyhow::bail!("UEFI bundle missing RPI_EFI.fd after extraction");
    }

    Ok(uefi_dir)
}

pub(crate) fn cleanup_fedora_artifacts(base: &Path) {
    if let Some(spec) = crate::downloader::DOWNLOAD_INDEX.images.iter().find(|img| {
        img.os == OsKind::Fedora && img.variant == "kde_mobile_disk" && img.arch == "aarch64"
    }) {
        let xz = base.join(&spec.file_name);
        let raw = if spec.file_name.ends_with(".xz") {
            base.join(spec.file_name.trim_end_matches(".xz"))
        } else {
            base.join(format!("{}.raw", spec.file_name))
        };
        let _ = fs::remove_file(xz);
        let _ = fs::remove_file(raw);
    }
}

pub(crate) fn cleanup_uefi_artifacts(base: &Path) {
    let _ = fs::remove_dir_all(base);
}
