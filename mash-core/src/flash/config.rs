use super::cancel::cancel_requested;
use anyhow::{bail, Result};
use log::info;
use std::path::{Path, PathBuf};
use std::sync::mpsc::SyncSender;
use std::sync::Arc;

use crate::cli::PartitionScheme;
use crate::config_states::{HasRunMode, ValidateConfig};
use crate::install_report::{DiskIdentityReport, InstallReportWriter};
use crate::locale::LocaleConfig;
use crate::progress::{Phase, ProgressUpdate};
use mash_hal::InstallerHal;

/// Detected btrfs subvolumes in source image.
pub(super) struct BtrfsSubvols {
    pub(super) has_root: bool,
    pub(super) has_home: bool,
    pub(super) has_var: bool,
}

/// Installation context with all configuration.
pub struct FlashContext {
    pub hal: Arc<dyn InstallerHal>,
    pub image: PathBuf,
    pub disk: String,
    pub scheme: PartitionScheme,
    pub uefi_dir: PathBuf,
    pub dry_run: bool,
    pub auto_unmount: bool,
    pub locale: Option<LocaleConfig>,
    pub early_ssh: bool,
    pub progress_tx: Option<SyncSender<ProgressUpdate>>,
    pub work_dir: PathBuf,
    pub loop_device: Option<String>,
    pub efi_size: String,
    pub boot_size: String,
    pub root_end: String,
    pub report: Option<InstallReportWriter>,
}

impl FlashContext {
    pub(super) fn send_progress(&self, update: ProgressUpdate) {
        if let Some(ref report) = self.report {
            report.record_progress_update(&update);
        }
        if let Some(ref tx) = self.progress_tx {
            let _ = tx.send(update);
        }
    }

    pub(super) fn check_cancel(&self) -> Result<()> {
        if cancel_requested() {
            self.status("🧹 Cleaning up...");
            bail!("Cancelled");
        }
        Ok(())
    }

    pub(super) fn start_phase(&self, phase: Phase) {
        info!("📍 Starting phase: {}", phase.name());
        self.send_progress(ProgressUpdate::PhaseStarted(phase));
    }

    pub(super) fn complete_phase(&self, phase: Phase) {
        info!("✅ Completed phase: {}", phase.name());
        self.send_progress(ProgressUpdate::PhaseCompleted(phase));
    }

    pub(super) fn status(&self, msg: &str) {
        info!("{}", msg);
        self.send_progress(ProgressUpdate::Status(msg.to_string()));
    }

    /// Get partition device path (handles nvme/mmcblk naming).
    pub(super) fn partition_path(&self, num: u32) -> String {
        mash_hal::path::partition_path(&self.disk, num)
    }
}

/// Core flashing configuration.
///
/// This is the validated, non-UI-specific input required to execute a flash.
/// (UI-only selections like "download Fedora" belong in the TUI layer.)
#[derive(Debug, Clone)]
pub struct FlashConfig {
    /// Human label for the selected OS (for reporting / completion messaging).
    pub os_distro: Option<String>,
    /// Human label for the selected flavour/variant (for reporting / completion messaging).
    pub os_flavour: Option<String>,
    /// Selected target disk identity (best-effort; may be missing on some hardware).
    pub disk_identity: Option<DiskIdentityReport>,
    /// Where EFI came from (e.g. "download" or "local") for reporting.
    pub efi_source: Option<String>,
    pub image: PathBuf,
    pub disk: String,
    pub scheme: PartitionScheme,
    pub uefi_dir: PathBuf,
    pub dry_run: bool,
    pub auto_unmount: bool,
    pub locale: Option<LocaleConfig>,
    pub early_ssh: bool,
    pub progress_tx: Option<SyncSender<ProgressUpdate>>,
    pub efi_size: String,
    pub boot_size: String,
    pub root_end: String,
}

impl FlashConfig {
    pub fn validate(&self) -> Result<()> {
        if !self.image.exists() {
            bail!("Image file not found: {}", self.image.display());
        }
        if !self.uefi_dir.exists() {
            bail!("UEFI directory not found: {}", self.uefi_dir.display());
        }

        // Check for required UEFI file. Allow either:
        // - a directory containing RPI_EFI.fd (bundle), or
        // - a direct path to an EFI image file (will be staged into a temp dir at runtime)
        if self.uefi_dir.is_dir() {
            let rpi_efi = self.uefi_dir.join("RPI_EFI.fd");
            if !rpi_efi.exists() {
                bail!("Missing required UEFI file: {}", rpi_efi.display());
            }
        } else if self.uefi_dir.is_file() {
            // File path is accepted here; it will be normalized in `run_with_progress`.
        } else {
            bail!(
                "UEFI path is neither a file nor directory: {}",
                self.uefi_dir.display()
            );
        }

        let disk = super::runner::normalize_disk(&self.disk);
        if !Path::new(&disk).exists() {
            bail!("Disk device not found: {}", disk);
        }

        Ok(())
    }
}

impl ValidateConfig for FlashConfig {
    fn validate_cfg(&self) -> Result<()> {
        self.validate()
    }
}

impl HasRunMode for FlashConfig {
    fn is_dry_run(&self) -> bool {
        self.dry_run
    }
}
