use crate::downloader;
use crate::stage_runner::{StageDefinition, StageRunner};
use crate::state_manager::{self, DownloadArtifact};
use crate::system_config::packages::PackageManager;
use crate::{boot_config, disk_ops, preflight, system_config};
use anyhow::Result;
use std::fmt;
use std::path::PathBuf;
use std::sync::Arc;

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
    fn from_install_config(cfg: &InstallConfig) -> Self {
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

#[derive(Debug, Clone)]
pub struct InstallConfig {
    pub dry_run: bool,
    pub execute: bool,
    pub confirmed: bool,
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

#[derive(Debug, Clone)]
pub struct MountSpec {
    pub device: String,
    pub target: String,
    pub fstype: Option<String>,
}

#[derive(Debug, Clone)]
pub struct StagePlan {
    pub name: &'static str,
    pub description: String,
}

#[derive(Debug, Clone)]
pub struct InstallPlan {
    pub stages: Vec<StagePlan>,
    pub reboot_count: u32,
}

impl InstallPlan {
    pub fn summary_lines(&self) -> Vec<String> {
        let mut lines = Vec::new();
        lines.push("Execution plan:".to_string());
        for (idx, stage) in self.stages.iter().enumerate() {
            lines.push(format!(
                "{:02}. {} — {}",
                idx + 1,
                stage.name,
                stage.description
            ));
        }
        lines.push(format!("Reboots: {}", self.reboot_count));
        lines
    }
}

pub fn build_plan(cfg: &InstallConfig) -> InstallPlan {
    let disk = cfg.disk.as_deref().unwrap_or("<unspecified>").to_string();
    let mount_count = cfg.mounts.len();
    let format_count = cfg.format_ext4.len() + cfg.format_btrfs.len();
    let package_count = cfg.packages.len();

    let download_desc = format!(
        "Download Fedora {} {} + optional UEFI to {}",
        cfg.image_version,
        cfg.image_edition,
        cfg.mash_root.join("downloads").display()
    );
    let stages = vec![
        StagePlan {
            name: "Preflight",
            description: "Read-only checks".to_string(),
        },
        StagePlan {
            name: "Download assets",
            description: download_desc,
        },
        StagePlan {
            name: "Disk probe",
            description: format!("Target: {}", disk),
        },
        StagePlan {
            name: "Mount plan",
            description: format!("{} mount operations", mount_count),
        },
        StagePlan {
            name: "Format plan",
            description: format!("{} format operations", format_count),
        },
        StagePlan {
            name: "Package plan",
            description: format!("{} packages", package_count),
        },
        StagePlan {
            name: "Kernel fix check",
            description: if cfg.kernel_fix {
                "USB-root alignment enabled (paths required)".to_string()
            } else {
                "Skipped".to_string()
            },
        },
    ];

    InstallPlan {
        stages,
        reboot_count: cfg.reboot_count,
    }
}

fn run_download_stage(
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

pub fn run_pipeline(cfg: &InstallConfig) -> Result<InstallPlan> {
    let plan = build_plan(cfg);
    if cfg.execute && !cfg.confirmed {
        anyhow::bail!("Execution requires explicit confirmation");
    }
    let preflight_cfg = Arc::new(preflight::PreflightConfig::for_install(
        cfg.disk.as_ref().map(PathBuf::from),
    ));
    let stage_defs = plan
        .stages
        .iter()
        .map(|stage| match stage.name {
            "Preflight" => {
                let cfg = Arc::clone(&preflight_cfg);
                StageDefinition {
                    name: stage.name,
                    run: Box::new(move |_state, _dry_run| preflight::run(&cfg)),
                }
            }
            "Download assets" => {
                let download_cfg = DownloadStageConfig::from_install_config(cfg);
                StageDefinition {
                    name: stage.name,
                    run: Box::new(move |state, dry_run| {
                        run_download_stage(state, &download_cfg, dry_run)
                    }),
                }
            }
            name => {
                let description = stage.description.clone();
                StageDefinition {
                    name,
                    run: Box::new(move |_state, dry_run| {
                        if dry_run {
                            log::info!("DRY RUN: {} — {}", name, description);
                        } else {
                            log::info!("Stage: {} — {}", name, description);
                        }
                        Ok(())
                    }),
                }
            }
        })
        .collect::<Vec<_>>();

    let runner = if cfg.execute {
        StageRunner::new(cfg.state_path.clone(), cfg.dry_run)
    } else {
        StageRunner::new_with_persist(cfg.state_path.clone(), cfg.dry_run, false)
    };
    let _ = runner.run(&stage_defs)?;

    if !cfg.execute {
        return Ok(plan);
    }

    if cfg.dry_run {
        return Ok(plan);
    }

    let format_opts = disk_ops::format::FormatOptions::new(cfg.dry_run, cfg.confirmed);
    for device in &cfg.format_ext4 {
        disk_ops::format::format_ext4(PathBuf::from(device).as_path(), &format_opts)?;
    }
    for device in &cfg.format_btrfs {
        disk_ops::format::format_btrfs(PathBuf::from(device).as_path(), &format_opts)?;
    }

    for spec in &cfg.mounts {
        let target = PathBuf::from(&spec.target);
        std::fs::create_dir_all(&target)?;
        disk_ops::mounts::mount_device(
            PathBuf::from(&spec.device).as_path(),
            &target,
            spec.fstype.as_deref(),
            nix::mount::MsFlags::empty(),
            cfg.dry_run,
        )?;
    }

    let pkg_mgr = system_config::packages::DnfShell::new(cfg.dry_run);
    pkg_mgr.update()?;
    pkg_mgr.install(&cfg.packages)?;

    if cfg.kernel_fix {
        if let (Some(root), Some(mountinfo), Some(by_uuid)) = (
            cfg.kernel_fix_root.as_ref(),
            cfg.mountinfo_path.as_ref(),
            cfg.by_uuid_path.as_ref(),
        ) {
            let mountinfo_content = std::fs::read_to_string(mountinfo)?;
            boot_config::usb_root_fix::apply_usb_root_fix(root, &mountinfo_content, by_uuid)?;
        } else {
            log::warn!("Kernel fix enabled but required paths are missing.");
        }
    }

    Ok(plan)
}

impl fmt::Display for InstallPlan {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for line in self.summary_lines() {
            writeln!(f, "{}", line)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state_manager::{self, DownloadArtifact};
    use httpmock::Method::GET;
    use httpmock::MockServer;
    use sha2::{Digest, Sha256};
    use std::fs;
    use std::path::PathBuf;
    use tempfile::tempdir;

    #[test]
    fn plan_includes_expected_stages() {
        let cfg = InstallConfig {
            dry_run: true,
            execute: false,
            confirmed: false,
            state_path: PathBuf::from("/tmp/state.json"),
            disk: Some("/dev/sda".to_string()),
            mounts: Vec::new(),
            format_ext4: Vec::new(),
            format_btrfs: Vec::new(),
            packages: Vec::new(),
            kernel_fix: false,
            kernel_fix_root: None,
            mountinfo_path: None,
            by_uuid_path: None,
            reboot_count: 1,
            mash_root: PathBuf::from("/"),
            download_image: false,
            download_uefi: false,
            image_version: "43".to_string(),
            image_edition: "KDE".to_string(),
            download_mirror: None,
            download_checksum: None,
            download_checksum_url: None,
            download_timeout_secs: 120,
            download_retries: 3,
            download_dir: PathBuf::from("downloads/images"),
        };
        let plan = build_plan(&cfg);
        assert_eq!(plan.stages.len(), 7);
        assert_eq!(plan.stages[0].name, "Preflight");
        assert_eq!(plan.stages[1].name, "Download assets");
        assert_eq!(plan.stages[6].name, "Kernel fix check");
    }

    fn make_download_config(
        state_path: PathBuf,
        mash_root: PathBuf,
        download_dir: PathBuf,
        mirror: String,
        checksum: String,
    ) -> InstallConfig {
        InstallConfig {
            dry_run: false,
            execute: true,
            confirmed: true,
            state_path,
            disk: Some("/dev/sda".to_string()),
            mounts: Vec::new(),
            format_ext4: Vec::new(),
            format_btrfs: Vec::new(),
            packages: Vec::new(),
            kernel_fix: false,
            kernel_fix_root: None,
            mountinfo_path: None,
            by_uuid_path: None,
            reboot_count: 1,
            mash_root,
            download_image: true,
            download_uefi: false,
            image_version: "43".to_string(),
            image_edition: "KDE".to_string(),
            download_mirror: Some(mirror),
            download_checksum: Some(checksum),
            download_checksum_url: None,
            download_timeout_secs: 5,
            download_retries: 2,
            download_dir,
        }
    }

    #[test]
    fn download_stage_records_artifact() {
        let server = MockServer::start();
        let body = b"artifact";
        let checksum = format!("{:x}", Sha256::digest(body));
        server.mock(|when, then| {
            when.method(GET).path("/image");
            then.status(200).body(body);
        });

        let tmp = tempdir().unwrap();
        let state_path = tmp.path().join("state.json");
        let downloads = tmp.path().join("downloads").join("images");
        let cfg = make_download_config(
            state_path.clone(),
            tmp.path().to_path_buf(),
            downloads.clone(),
            server.url("/image"),
            checksum.clone(),
        );

        run_pipeline(&cfg).unwrap();

        let saved = state_manager::load_state(&state_path).unwrap().unwrap();
        assert!(saved
            .completed_stages
            .contains(&"Download assets".to_string()));
        assert_eq!(saved.download_artifacts.len(), 1);
        assert_eq!(saved.verified_checksums, vec![checksum]);
        assert_eq!(saved.download_artifacts[0].size, body.len() as u64);
    }

    #[test]
    fn download_stage_skips_when_completed() {
        let tmp = tempdir().unwrap();
        let state_path = tmp.path().join("state.json");
        let downloads = tmp.path().join("downloads").join("images");
        fs::create_dir_all(&downloads).unwrap();
        let artifact_path = downloads.join("Fedora-43-KDE-aarch64.raw.xz");
        fs::write(&artifact_path, b"cached").unwrap();
        let checksum = format!("{:x}", Sha256::digest(b"cached"));

        let mut preset = state_manager::InstallState::new(false);
        preset.record_download(DownloadArtifact::new(
            "Fedora-43-KDE-aarch64.raw.xz".to_string(),
            &artifact_path,
            6,
            checksum.clone(),
            false,
        ));
        preset.mark_checksum_verified(&checksum);
        preset.mark_completed("Download assets");
        state_manager::save_state_atomic(&state_path, &preset).unwrap();

        let cfg = make_download_config(
            state_path.clone(),
            tmp.path().to_path_buf(),
            downloads.clone(),
            "http://127.0.0.1/unused".to_string(),
            checksum,
        );

        run_pipeline(&cfg).unwrap();

        let saved = state_manager::load_state(&state_path).unwrap().unwrap();
        assert_eq!(saved.download_artifacts.len(), 1);
        assert!(saved
            .completed_stages
            .contains(&"Download assets".to_string()));
    }

    #[test]
    fn download_stage_checksum_failure() {
        let server = MockServer::start();
        server.mock(|when, then| {
            when.method(GET).path("/image");
            then.status(200).body(b"bad");
        });

        let tmp = tempdir().unwrap();
        let state_path = tmp.path().join("state.json");
        let downloads = tmp.path().join("downloads").join("images");
        let cfg = make_download_config(
            state_path.clone(),
            tmp.path().to_path_buf(),
            downloads.clone(),
            server.url("/image"),
            "deadbeef".to_string(),
        );

        assert!(run_pipeline(&cfg).is_err());
        let saved = state_manager::load_state(&state_path).unwrap().unwrap();
        assert!(!saved
            .completed_stages
            .contains(&"Download assets".to_string()));
        assert!(saved.download_artifacts.is_empty());
    }
}
