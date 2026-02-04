use super::config::{
    BootStageConfig, DiskStageConfig, DownloadStageConfig, MountStageConfig, PackageStageConfig,
    ResumeStageConfig,
};
use anyhow::{Context, Result};
use mash_core::downloader;
use mash_core::state_manager::{self, DownloadArtifact};
use mash_core::{boot_config, system_config};
use std::env;
use std::path::PathBuf;

pub(super) fn run_download_stage(
    state: &mut state_manager::InstallState,
    cfg: &DownloadStageConfig,
    dry_run: bool,
) -> Result<()> {
    if !cfg.enabled {
        log::info!("Download stage skipped; download_image disabled");
        return Ok(());
    }
    if dry_run {
        log::info!("DRY RUN: download stage would fetch Fedora assets");
        return Ok(());
    }

    let opts = downloader::DownloadOptions {
        asset: None,
        image: Some(downloader::ImageKey {
            os: downloader::OsKind::Fedora,
            // This pipeline is currently Fedora-oriented; default to the canonical Fedora entry.
            variant: "kde_mobile_disk".to_string(),
            arch: "aarch64".to_string(),
        }),
        mirror_override: cfg.mirror_override.clone(),
        checksum_override: cfg.checksum_override.clone(),
        checksum_url: cfg.checksum_url.clone(),
        max_retries: cfg.retries,
        timeout_secs: cfg.timeout_secs,
        download_dir: cfg.download_dir.clone(),
        resume: true,
    };
    let artifact = downloader::download(&opts)?;
    state.record_download(DownloadArtifact::new(
        artifact.name.clone(),
        &artifact.path,
        artifact.size,
        artifact.checksum.clone(),
        artifact.resumed,
    ));
    state.mark_checksum_verified(&artifact.checksum);
    state.set_partial_resume(artifact.resumed);
    Ok(())
}

pub(super) fn run_disk_stage<H: mash_hal::FormatOps>(
    hal: &H,
    state: &mut state_manager::InstallState,
    cfg: &DiskStageConfig,
    dry_run: bool,
) -> Result<()> {
    if cfg.format_ext4.is_empty() && cfg.format_btrfs.is_empty() {
        log::info!("Disk stage skipped; no format targets configured");
        return Ok(());
    }
    let format_opts = mash_hal::FormatOptions::new(dry_run, true);
    for device in &cfg.format_ext4 {
        hal.format_ext4(device, &format_opts)?;
        if !dry_run {
            state.record_formatted_device(device);
        }
    }
    for device in &cfg.format_btrfs {
        hal.format_btrfs(device, &format_opts)?;
        if !dry_run {
            state.record_formatted_device(device);
        }
    }
    Ok(())
}

pub(super) fn run_boot_stage(
    state: &mut state_manager::InstallState,
    cfg: &BootStageConfig,
    dry_run: bool,
) -> Result<()> {
    if !cfg.enabled {
        log::info!("Boot stage skipped; kernel fix disabled");
        return Ok(());
    }
    let root = cfg
        .root
        .as_ref()
        .context("kernel_fix_root is required for boot stage")?;
    let mountinfo_path = cfg
        .mountinfo
        .as_ref()
        .context("mountinfo_path is required for boot stage")?;
    let by_uuid_path = cfg
        .by_uuid
        .as_ref()
        .context("by_uuid_path is required for boot stage")?;
    if dry_run {
        log::info!(
            "DRY RUN: kernel fix would patch {} using {} and {}",
            root.display(),
            mountinfo_path.display(),
            by_uuid_path.display()
        );
        return Ok(());
    }
    let mountinfo_content = std::fs::read_to_string(mountinfo_path)?;
    boot_config::usb_root_fix::apply_usb_root_fix(root, &mountinfo_content, by_uuid_path)?;
    state.mark_boot_completed();
    Ok(())
}

pub(super) fn run_mount_stage<H: mash_hal::MountOps>(
    hal: &H,
    _state: &mut state_manager::InstallState,
    cfg: &MountStageConfig,
    dry_run: bool,
) -> Result<()> {
    if cfg.mounts.is_empty() {
        log::info!("Mount stage skipped; no mounts configured");
        return Ok(());
    }

    for spec in &cfg.mounts {
        let target = PathBuf::from(&spec.target);
        if hal.is_mounted(&target).unwrap_or(false) {
            log::info!("Mount already present: {}", target.display());
            continue;
        }
        if !dry_run {
            std::fs::create_dir_all(&target)?;
        }
        hal.mount_device(
            PathBuf::from(&spec.device).as_path(),
            &target,
            spec.fstype.as_deref(),
            mash_hal::MountOptions::new(),
            dry_run,
        )?;
    }
    Ok(())
}

pub(super) fn run_package_stage(
    _state: &mut state_manager::InstallState,
    cfg: &PackageStageConfig,
    dry_run: bool,
) -> Result<()> {
    if cfg.packages.is_empty() {
        log::info!("Package stage skipped; no packages configured");
        return Ok(());
    }
    let pkg_mgr = system_config::packages::default_package_manager(dry_run);
    pkg_mgr.update()?;
    pkg_mgr.install(&cfg.packages)?;
    Ok(())
}

pub(super) fn run_resume_stage(
    state: &mut state_manager::InstallState,
    cfg: &ResumeStageConfig,
    dry_run: bool,
) -> Result<()> {
    if !state.boot_stage_completed {
        log::info!("Resume stage skipped; boot stage not completed");
        return Ok(());
    }

    if dry_run {
        log::info!("DRY RUN: would install resume unit + request reboot");
        return Ok(());
    }

    let exec_path = env::current_exe().context("Failed to determine current executable path")?;
    let unit_content = system_config::resume::render_resume_unit(&exec_path, &cfg.state_path);
    system_config::resume::install_resume_unit(&cfg.mash_root, &unit_content)?;
    if let Some(conn) = system_config::resume::connect_systemd() {
        system_config::resume::enable_resume_unit(&conn)?;
    } else {
        log::warn!("No systemd connection available; skipping resume unit enable");
    }
    system_config::resume::request_reboot(dry_run)?;

    Ok(())
}
