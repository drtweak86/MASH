use anyhow::Result;
use mash_core::config_states::{HasRunMode, ValidateConfig};
use std::path::PathBuf;

#[derive(Clone)]
pub struct DownloadStageConfig {
    pub enabled: bool,
    pub mirror_override: Option<String>,
    pub checksum_override: Option<String>,
    pub checksum_url: Option<String>,
    pub timeout_secs: u64,
    pub retries: usize,
    pub download_dir: PathBuf,
}

impl DownloadStageConfig {
    pub(super) fn from_install_config(cfg: &InstallConfig) -> Self {
        Self {
            enabled: cfg.download_image,
            mirror_override: cfg.download_mirror.clone(),
            checksum_override: cfg.download_checksum.clone(),
            checksum_url: cfg.download_checksum_url.clone(),
            timeout_secs: cfg.download_timeout_secs,
            retries: cfg.download_retries,
            download_dir: cfg.download_dir.clone(),
        }
    }
}

#[derive(Clone)]
pub struct DiskStageConfig {
    pub format_ext4: Vec<PathBuf>,
    pub format_btrfs: Vec<PathBuf>,
}

impl DiskStageConfig {
    pub(super) fn from_install_config(cfg: &InstallConfig) -> Self {
        Self {
            format_ext4: cfg.format_ext4.iter().map(PathBuf::from).collect(),
            format_btrfs: cfg.format_btrfs.iter().map(PathBuf::from).collect(),
        }
    }
}

#[derive(Clone)]
pub struct BootStageConfig {
    pub enabled: bool,
    pub root: Option<PathBuf>,
    pub mountinfo: Option<PathBuf>,
    pub by_uuid: Option<PathBuf>,
}

impl BootStageConfig {
    pub(super) fn from_install_config(cfg: &InstallConfig) -> Self {
        Self {
            enabled: cfg.kernel_fix,
            root: cfg.kernel_fix_root.clone(),
            mountinfo: cfg.mountinfo_path.clone(),
            by_uuid: cfg.by_uuid_path.clone(),
        }
    }
}

#[derive(Clone)]
pub struct MountStageConfig {
    pub mounts: Vec<MountSpec>,
}

impl MountStageConfig {
    pub(super) fn from_install_config(cfg: &InstallConfig) -> Self {
        Self {
            mounts: cfg.mounts.clone(),
        }
    }
}

#[derive(Clone)]
pub struct PackageStageConfig {
    pub packages: Vec<String>,
}

impl PackageStageConfig {
    pub(super) fn from_install_config(cfg: &InstallConfig) -> Self {
        Self {
            packages: cfg.packages.clone(),
        }
    }
}

#[derive(Clone)]
pub struct ResumeStageConfig {
    pub mash_root: PathBuf,
    pub state_path: PathBuf,
}

impl ResumeStageConfig {
    pub(super) fn from_install_config(cfg: &InstallConfig) -> Self {
        Self {
            mash_root: cfg.mash_root.clone(),
            state_path: cfg.state_path.clone(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct InstallConfig {
    pub dry_run: bool,
    pub execute: bool,
    pub state_path: PathBuf,
    pub disk: Option<String>,
    pub mounts: Vec<MountSpec>,
    pub format_ext4: Vec<String>,
    pub format_btrfs: Vec<String>,
    pub packages: Vec<String>,
    pub kernel_fix: bool,
    pub kernel_fix_root: Option<PathBuf>,
    pub mountinfo_path: Option<PathBuf>,
    pub by_uuid_path: Option<PathBuf>,
    pub reboot_count: u32,
    pub mash_root: PathBuf,
    pub download_image: bool,
    pub download_uefi: bool,
    pub image_version: String,
    pub image_edition: String,
    pub download_mirror: Option<String>,
    pub download_checksum: Option<String>,
    pub download_checksum_url: Option<String>,
    pub download_timeout_secs: u64,
    pub download_retries: usize,
    pub download_dir: PathBuf,
}

impl ValidateConfig for InstallConfig {
    fn validate_cfg(&self) -> Result<()> {
        // Keep validation lightweight here; preflight does the heavy lifting.
        Ok(())
    }
}

impl HasRunMode for InstallConfig {
    fn is_dry_run(&self) -> bool {
        self.dry_run
    }
}

#[derive(Debug, Clone)]
pub struct MountSpec {
    pub device: String,
    pub target: String,
    pub fstype: Option<String>,
}
