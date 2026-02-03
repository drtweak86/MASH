use crate::install_runner::{StageDefinition, StageRunner};
use anyhow::{Context, Result};
use mash_core::downloader::{self, DownloadOptions, ImageKey, OsKind};
use mash_core::progress::ProgressUpdate;
use mash_core::state_manager::{save_state_atomic, DownloadArtifact, InstallState};
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::sync::mpsc::Sender;

/// Where the OS image comes from.
#[derive(Debug, Clone)]
pub enum ImageSource {
    /// Use a local on-disk image (typically .img, .raw, or compressed .xz).
    Local(PathBuf),
    /// Download the image from the canonical index (docs/os-download-links.toml).
    Download,
}

#[derive(Debug, Clone)]
pub struct OsInstallConfig {
    pub mash_root: PathBuf,
    pub state_path: PathBuf,
    pub os: OsKind,
    pub variant: String,
    pub arch: String,
    pub target_disk: PathBuf,
    pub download_dir: PathBuf,
    pub image_source: ImageSource,
    pub dry_run: bool,
    pub progress_tx: Option<Sender<ProgressUpdate>>,
}

impl OsInstallConfig {
    pub fn image_key(&self) -> ImageKey {
        ImageKey {
            os: self.os,
            variant: self.variant.clone(),
            arch: self.arch.clone(),
        }
    }
}

pub fn run<H>(
    hal: &H,
    cfg: &OsInstallConfig,
    destructive_confirmed: bool,
    typed_confirmation: bool,
    cancel: Option<&AtomicBool>,
) -> Result<InstallState>
where
    H: mash_hal::FlashOps + Send + Sync + Clone + 'static,
{
    if !cfg.dry_run && !destructive_confirmed {
        anyhow::bail!("refusing to run destructive install without confirmation");
    }

    let state_path = cfg.state_path.clone();
    let runner = StageRunner::new(state_path.clone(), cfg.dry_run);

    let mut stages: Vec<StageDefinition<'_>> = Vec::new();

    // Persistent install report artifact (always).
    let mode = if cfg.dry_run {
        mash_core::install_report::RunMode::DryRun
    } else {
        mash_core::install_report::RunMode::Execute
    };
    let selection = mash_core::install_report::SelectionReport {
        distro: format!("{:?}", cfg.os),
        flavour: Some(cfg.variant.clone()),
        target_disk: cfg.target_disk.display().to_string(),
        disk_identity: None,
        partition_scheme: None,
        efi_size: None,
        boot_size: None,
        root_end: None,
        efi_source: None,
        efi_path: None,
    };
    let report = mash_core::install_report::InstallReportWriter::new(
        mode,
        destructive_confirmed,
        typed_confirmation,
        selection,
    )
    .ok();

    // Persist the install intent early so resumes are self-describing.
    let os = cfg.os;
    let variant = cfg.variant.clone();
    let progress_intent = cfg.progress_tx.clone();
    let report_intent = report.clone();
    stages.push(StageDefinition {
        name: "Record install intent",
        run: Box::new(move |state, _dry_run| {
            if let Some(ref report) = report_intent {
                report.stage_started("Record install intent");
            }
            if let Some(tx) = &progress_intent {
                let _ = tx.send(ProgressUpdate::Status(
                    "🧾 Recording install intent...".to_string(),
                ));
            }
            state.selected_os = Some(format!("{:?}", os));
            state.selected_variant = Some(variant.clone());
            if let Some(ref report) = report_intent {
                report.stage_completed("Record install intent");
            }
            Ok(())
        }),
    });

    // Download stage (optional).
    let cfg_dl = cfg.clone();
    let cancel_dl = cancel;
    let report_dl = report.clone();
    stages.push(StageDefinition {
        name: "Download OS image",
        run: Box::new(move |state, dry_run| {
            if let Some(ref report) = report_dl {
                report.stage_started("Download OS image");
            }
            if let Some(tx) = &cfg_dl.progress_tx {
                let _ = tx.send(ProgressUpdate::Status(
                    "⬇️ Downloading OS image...".to_string(),
                ));
            }
            if matches!(cfg_dl.image_source, ImageSource::Local(_)) {
                log::info!("Download stage skipped; using local image");
                if let Some(ref report) = report_dl {
                    report.stage_completed("Download OS image");
                }
                return Ok(());
            }
            if dry_run {
                log::info!(
                    "DRY RUN: would download {:?} {} ({}) to {}",
                    cfg_dl.os,
                    cfg_dl.variant,
                    cfg_dl.arch,
                    cfg_dl.download_dir.display()
                );
                if let Some(ref report) = report_dl {
                    report.stage_completed("Download OS image");
                }
                return Ok(());
            }
            if let Some(flag) = cancel_dl {
                if flag.load(std::sync::atomic::Ordering::Relaxed) {
                    if let Some(ref report) = report_dl {
                        report.stage_error("Download OS image", "cancelled");
                    }
                    anyhow::bail!("cancelled");
                }
            }
            let opts = DownloadOptions {
                image: Some(cfg_dl.image_key()),
                download_dir: cfg_dl.download_dir.clone(),
                ..Default::default()
            };
            let artifact = match downloader::download(&opts) {
                Ok(artifact) => artifact,
                Err(err) => {
                    if let Some(ref report) = report_dl {
                        report.stage_error("Download OS image", &err.to_string());
                    }
                    return Err(err);
                }
            };
            state.record_download(DownloadArtifact::new(
                artifact.name.clone(),
                &artifact.path,
                artifact.size,
                artifact.checksum.clone(),
                artifact.resumed,
            ));
            state.mark_checksum_verified(&artifact.checksum);
            state.set_partial_resume(artifact.resumed);
            if let Some(ref report) = report_dl {
                report.stage_completed("Download OS image");
            }
            Ok(())
        }),
    });

    // Flash stage.
    let cfg_flash = cfg.clone();
    let cancel_flash = cancel;
    let hal_flash = hal.clone();
    let report_flash = report.clone();
    stages.push(StageDefinition {
        name: "Flash OS image",
        run: Box::new(move |state, dry_run| {
            if let Some(ref report) = report_flash {
                report.stage_started("Flash OS image");
            }
            if let Some(tx) = &cfg_flash.progress_tx {
                let _ = tx.send(ProgressUpdate::Status(
                    "💾 Flashing image to disk...".to_string(),
                ));
            }
            if dry_run {
                log::info!(
                    "DRY RUN: would flash {:?} {} to {}",
                    cfg_flash.os,
                    cfg_flash.variant,
                    cfg_flash.target_disk.display()
                );
                if let Some(ref report) = report_flash {
                    report.stage_completed("Flash OS image");
                }
                return Ok(());
            }
            if let Some(flag) = cancel_flash {
                if flag.load(std::sync::atomic::Ordering::Relaxed) {
                    if let Some(ref report) = report_flash {
                        report.stage_error("Flash OS image", "cancelled");
                    }
                    anyhow::bail!("cancelled");
                }
            }

            let image_path = resolve_image_path(&cfg_flash, state)?;
            let opts = mash_hal::FlashOptions::new(dry_run, destructive_confirmed);
            if let Err(err) = hal_flash.flash_raw_image(&image_path, &cfg_flash.target_disk, &opts)
            {
                if let Some(ref report) = report_flash {
                    report.stage_error("Flash OS image", &err.to_string());
                }
                return Err(err);
            }

            if !state
                .flashed_devices
                .iter()
                .any(|p| p == &cfg_flash.target_disk.display().to_string())
            {
                state
                    .flashed_devices
                    .push(cfg_flash.target_disk.display().to_string());
            }
            if let Some(ref report) = report_flash {
                report.stage_completed("Flash OS image");
            }
            Ok(())
        }),
    });

    // OS-specific post-flash rules.
    let cfg_rules = cfg.clone();
    let report_rules = report.clone();
    stages.push(StageDefinition {
        name: "Apply OS-specific rules",
        run: Box::new(move |state, dry_run| {
            if let Some(ref report) = report_rules {
                report.stage_started("Apply OS-specific rules");
            }
            if let Some(tx) = &cfg_rules.progress_tx {
                let _ = tx.send(ProgressUpdate::Status(
                    "🔧 Applying OS rules...".to_string(),
                ));
            }
            if dry_run {
                if let Some(ref report) = report_rules {
                    report.stage_completed("Apply OS-specific rules");
                }
                return Ok(());
            }
            if cfg_rules.os == OsKind::Manjaro {
                state.post_boot_partition_expansion_required = true;
            }
            if let Some(ref report) = report_rules {
                report.stage_completed("Apply OS-specific rules");
            }
            Ok(())
        }),
    });

    // Ensure we write the final state even if the caller doesn't persist runner output.
    let final_state = runner.run(&stages)?;
    if let Some(ref report) = report {
        report.record_progress_update(&ProgressUpdate::Complete);
    }
    save_state_atomic(&state_path, &final_state)?;
    Ok(final_state)
}

fn resolve_image_path(cfg: &OsInstallConfig, state: &InstallState) -> Result<PathBuf> {
    match &cfg.image_source {
        ImageSource::Local(path) => Ok(path.clone()),
        ImageSource::Download => {
            let key = cfg.image_key();
            let expected_name = downloader::DOWNLOAD_INDEX
                .images
                .iter()
                .find(|s| s.os == key.os && s.variant == key.variant && s.arch == key.arch)
                .map(|s| s.file_name.clone())
                .context("download index missing image spec for selection")?;

            // Find an artifact matching the downloaded filename.
            let artifact = state
                .download_artifacts
                .iter()
                .rev()
                .find(|a| a.name == expected_name)
                .context("download artifact not found in state; cannot flash")?;

            let path = Path::new(&artifact.path);
            if !path.exists() {
                anyhow::bail!("downloaded image missing on disk: {}", artifact.path);
            }
            Ok(path.to_path_buf())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn xz_encode(data: &[u8]) -> Vec<u8> {
        use std::io::Write;
        let mut out = Vec::new();
        let mut enc = xz2::write::XzEncoder::new(&mut out, 6);
        enc.write_all(data).unwrap();
        enc.finish().unwrap();
        out
    }

    #[test]
    fn ubuntu_full_path_flashes_image_to_target() {
        let dir = tempdir().unwrap();
        let mash_root = dir.path().join("mash");
        let state_path = dir.path().join("state.json");
        let downloads = mash_root.join("downloads/images");
        let img_path = dir.path().join("ubuntu.img.xz");
        let disk_path = dir.path().join("disk.img");

        let content = b"ubuntu-image-content";
        std::fs::write(&img_path, xz_encode(content)).unwrap();

        let cfg = OsInstallConfig {
            mash_root,
            state_path: state_path.clone(),
            os: OsKind::Ubuntu,
            variant: "server_24_04_3".to_string(),
            arch: "aarch64".to_string(),
            target_disk: disk_path.clone(),
            download_dir: downloads,
            image_source: ImageSource::Local(img_path),
            dry_run: false,
            progress_tx: None,
        };

        let hal = mash_hal::LinuxHal::new();
        let state = run(&hal, &cfg, true, false, None).unwrap();
        assert!(state
            .flashed_devices
            .iter()
            .any(|p| p == &disk_path.display().to_string()));
        assert!(!state.post_boot_partition_expansion_required);

        let written = std::fs::read(&disk_path).unwrap();
        assert_eq!(written, content);
    }

    #[test]
    fn raspios_full_path_flashes_image_to_target() {
        let dir = tempdir().unwrap();
        let mash_root = dir.path().join("mash");
        let state_path = dir.path().join("state.json");
        let downloads = mash_root.join("downloads/images");
        let img_path = dir.path().join("raspios.img.xz");
        let disk_path = dir.path().join("disk.img");

        let content = b"raspios-image-content";
        std::fs::write(&img_path, xz_encode(content)).unwrap();

        let cfg = OsInstallConfig {
            mash_root,
            state_path,
            os: OsKind::RaspberryPiOS,
            variant: "arm64_latest".to_string(),
            arch: "aarch64".to_string(),
            target_disk: disk_path.clone(),
            download_dir: downloads,
            image_source: ImageSource::Local(img_path),
            dry_run: false,
            progress_tx: None,
        };

        let hal = mash_hal::LinuxHal::new();
        let state = run(&hal, &cfg, true, false, None).unwrap();
        assert!(!state.post_boot_partition_expansion_required);
        let written = std::fs::read(&disk_path).unwrap();
        assert_eq!(written, content);
    }

    #[test]
    fn manjaro_path_sets_post_boot_expansion_flag_and_does_not_error() {
        let dir = tempdir().unwrap();
        let mash_root = dir.path().join("mash");
        let state_path = dir.path().join("state.json");
        let downloads = mash_root.join("downloads/images");
        let img_path = dir.path().join("manjaro.img.xz");
        let disk_path = dir.path().join("disk.img");

        let content = b"manjaro-image";
        std::fs::write(&img_path, xz_encode(content)).unwrap();

        let cfg = OsInstallConfig {
            mash_root,
            state_path,
            os: OsKind::Manjaro,
            variant: "minimal_rpi4_23_02".to_string(),
            arch: "aarch64".to_string(),
            target_disk: disk_path.clone(),
            download_dir: downloads,
            image_source: ImageSource::Local(img_path),
            dry_run: false,
            progress_tx: None,
        };

        let hal = mash_hal::LinuxHal::new();
        let state = run(&hal, &cfg, true, false, None).unwrap();
        assert!(state.post_boot_partition_expansion_required);
        let written = std::fs::read(&disk_path).unwrap();
        assert_eq!(written, content);
    }
}
