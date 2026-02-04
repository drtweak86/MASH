use super::config::{
    BootStageConfig, DiskStageConfig, DownloadStageConfig, InstallConfig, MountStageConfig,
    PackageStageConfig, ResumeStageConfig,
};
use super::plan::InstallPlan;
use super::stages::{
    run_boot_stage, run_disk_stage, run_download_stage, run_mount_stage, run_package_stage,
    run_resume_stage,
};
use crate::install_runner::StageDefinition;
use crate::preflight;
use log;
use mash_core::config_states::ValidatedConfig;
use mash_core::state_manager::StageName;
use mash_hal::LinuxHal;
use std::sync::Arc;

pub fn build_stage_definitions(
    plan: &InstallPlan,
    cfg: &ValidatedConfig<InstallConfig>,
    hal: Arc<LinuxHal>,
) -> Vec<StageDefinition<'static>> {
    let preflight_cfg = crate::preflight::PreflightConfig::for_install(
        cfg.0.disk.as_ref().map(std::path::PathBuf::from),
        !cfg.0.packages.is_empty() || cfg.0.download_image || cfg.0.download_uefi,
        required_binaries(&cfg.0),
    );
    let download_cfg = DownloadStageConfig::from_install_config(&cfg.0);
    let mount_cfg = MountStageConfig::from_install_config(&cfg.0);
    let package_cfg = PackageStageConfig::from_install_config(&cfg.0);
    let resume_cfg = ResumeStageConfig::from_install_config(&cfg.0);
    let disk_cfg = DiskStageConfig::from_install_config(&cfg.0);
    let boot_cfg = BootStageConfig::from_install_config(&cfg.0);

    let mut defs = Vec::new();
    for stage in &plan.stages {
        let def = match stage.name {
            "Preflight" => {
                let cfg = preflight_cfg.clone();
                StageDefinition {
                    name: StageName::Preflight,
                    run: Box::new(move |_state, _dry_run| preflight::run(&cfg)),
                }
            }
            "Download assets" => {
                let cfg = download_cfg.clone();
                StageDefinition {
                    name: StageName::DownloadAssets,
                    run: Box::new(move |state, dry_run| run_download_stage(state, &cfg, dry_run)),
                }
            }
            "Mount plan" => {
                let cfg = mount_cfg.clone();
                let hal = Arc::clone(&hal);
                StageDefinition {
                    name: StageName::MountPlan,
                    run: Box::new(move |state, dry_run| {
                        run_mount_stage(hal.as_ref(), state, &cfg, dry_run)
                    }),
                }
            }
            "Format plan" => {
                let cfg = disk_cfg.clone();
                let hal = Arc::clone(&hal);
                StageDefinition {
                    name: StageName::FormatPlan,
                    run: Box::new(move |state, dry_run| {
                        run_disk_stage(hal.as_ref(), state, &cfg, dry_run)
                    }),
                }
            }
            "Package plan" => {
                let cfg = package_cfg.clone();
                StageDefinition {
                    name: StageName::PackagePlan,
                    run: Box::new(move |state, dry_run| run_package_stage(state, &cfg, dry_run)),
                }
            }
            "Kernel fix check" => {
                let cfg = boot_cfg.clone();
                StageDefinition {
                    name: StageName::KernelFixCheck,
                    run: Box::new(move |state, dry_run| run_boot_stage(state, &cfg, dry_run)),
                }
            }
            "Resume unit" => {
                let cfg = resume_cfg.clone();
                StageDefinition {
                    name: StageName::ResumeUnit,
                    run: Box::new(move |state, dry_run| run_resume_stage(state, &cfg, dry_run)),
                }
            }
            name => {
                let description = stage.description.clone();
                StageDefinition {
                    name: StageName::from(name),
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
        };
        defs.push(def);
    }
    defs
}

fn required_binaries(cfg: &InstallConfig) -> Vec<String> {
    let mut bins = Vec::new();
    if !cfg.packages.is_empty() {
        bins.push("dnf".to_string());
    }
    if !cfg.format_ext4.is_empty() {
        bins.push("mkfs.ext4".to_string());
    }
    if !cfg.format_btrfs.is_empty() {
        bins.push("mkfs.btrfs".to_string());
    }
    bins
}
