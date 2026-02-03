use crate::downloader::{self, DownloadOptions, ImageKey, OsKind};
use crate::stage_runner::{StageDefinition, StageRunner};
use crate::state_manager::{save_state_atomic, InstallState};
use crate::tui::progress::ProgressUpdate;
use anyhow::{Context, Result};
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

pub fn run(
    cfg: &OsInstallConfig,
    destructive_confirmed: bool,
    cancel: Option<&AtomicBool>,
) -> Result<InstallState> {
    if !cfg.dry_run && !destructive_confirmed {
        anyhow::bail!("refusing to run destructive install without confirmation");
    }

    let state_path = cfg.state_path.clone();
    let runner = StageRunner::new(state_path.clone(), cfg.dry_run);

    let mut stages: Vec<StageDefinition<'_>> = Vec::new();

    // Persist the install intent early so resumes are self-describing.
    let os = cfg.os;
    let variant = cfg.variant.clone();
    let progress_intent = cfg.progress_tx.clone();
    stages.push(StageDefinition {
        name: "Record install intent",
        run: Box::new(move |state, _dry_run| {
            if let Some(tx) = &progress_intent {
                let _ = tx.send(ProgressUpdate::Status(
                    "🧾 Recording install intent...".to_string(),
                ));
            }
            state.selected_os = Some(format!("{:?}", os));
            state.selected_variant = Some(variant.clone());
            Ok(())
        }),
    });

    // Download stage (optional).
    let cfg_dl = cfg.clone();
    let cancel_dl = cancel;
    stages.push(StageDefinition {
        name: "Download OS image",
        run: Box::new(move |state, dry_run| {
            if let Some(tx) = &cfg_dl.progress_tx {
                let _ = tx.send(ProgressUpdate::Status(
                    "⬇️ Downloading OS image...".to_string(),
                ));
            }
            if matches!(cfg_dl.image_source, ImageSource::Local(_)) {
                log::info!("Download stage skipped; using local image");
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
                return Ok(());
            }
            if let Some(flag) = cancel_dl {
                if flag.load(std::sync::atomic::Ordering::Relaxed) {
                    anyhow::bail!("cancelled");
                }
            }
            let opts = DownloadOptions {
                image: Some(cfg_dl.image_key()),
                download_dir: cfg_dl.download_dir.clone(),
                ..Default::default()
            };
            let artifact = downloader::download(&opts)?;
            state.record_download(crate::state_manager::DownloadArtifact::new(
                artifact.name.clone(),
                &artifact.path,
                artifact.size,
                artifact.checksum.clone(),
                artifact.resumed,
            ));
            state.mark_checksum_verified(&artifact.checksum);
            state.set_partial_resume(artifact.resumed);
            Ok(())
        }),
    });

    // Flash stage.
    let cfg_flash = cfg.clone();
    let cancel_flash = cancel;
    stages.push(StageDefinition {
        name: "Flash OS image",
        run: Box::new(move |state, dry_run| {
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
                return Ok(());
            }
            if let Some(flag) = cancel_flash {
                if flag.load(std::sync::atomic::Ordering::Relaxed) {
                    anyhow::bail!("cancelled");
                }
            }

            let image_path = resolve_image_path(&cfg_flash, state)?;
            crate::flash::flash_raw_image_to_disk(&image_path, &cfg_flash.target_disk)?;

            if !state
                .flashed_devices
                .iter()
                .any(|p| p == &cfg_flash.target_disk.display().to_string())
            {
                state
                    .flashed_devices
                    .push(cfg_flash.target_disk.display().to_string());
            }
            Ok(())
        }),
    });

    // OS-specific post-flash rules.
    let cfg_rules = cfg.clone();
    stages.push(StageDefinition {
        name: "Apply OS-specific rules",
        run: Box::new(move |state, dry_run| {
            if let Some(tx) = &cfg_rules.progress_tx {
                let _ = tx.send(ProgressUpdate::Status(
                    "🔧 Applying OS rules...".to_string(),
                ));
            }
            if dry_run {
                return Ok(());
            }
            if cfg_rules.os == OsKind::Manjaro {
                state.post_boot_partition_expansion_required = true;
            }
            Ok(())
        }),
    });

    // Ensure we write the final state even if the caller doesn't persist runner output.
    let final_state = runner.run(&stages)?;
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

        let state = run(&cfg, true, None).unwrap();
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

        let state = run(&cfg, true, None).unwrap();
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

        let state = run(&cfg, true, None).unwrap();
        assert!(state.post_boot_partition_expansion_required);
        let written = std::fs::read(&disk_path).unwrap();
        assert_eq!(written, content);
    }
}
