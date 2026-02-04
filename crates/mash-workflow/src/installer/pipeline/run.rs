use super::config::{
    BootStageConfig, DiskStageConfig, DownloadStageConfig, InstallConfig, MountStageConfig,
    PackageStageConfig, ResumeStageConfig,
};
use super::plan::{build_plan, InstallPlan};
use super::stages::{
    run_boot_stage, run_disk_stage, run_download_stage, run_mount_stage, run_package_stage,
    run_resume_stage,
};
use crate::install_runner::{StageDefinition, StageRunner};
use crate::preflight;
use anyhow::Result;
use mash_core::config_states::{ArmedConfig, ValidatedConfig};
use std::path::PathBuf;
use std::sync::Arc;

pub fn run_pipeline(cfg: &InstallConfig) -> Result<InstallPlan> {
    let validated = mash_core::config_states::UnvalidatedConfig::new(cfg.clone()).validate()?;
    run_pipeline_validated(validated)
}

pub fn run_pipeline_execute(cfg: ArmedConfig<InstallConfig>) -> Result<InstallPlan> {
    if !cfg.cfg.execute || cfg.cfg.dry_run {
        anyhow::bail!("run_pipeline_execute requires execute=true and dry_run=false");
    }
    run_pipeline_impl(cfg.cfg)
}

fn run_pipeline_validated(cfg: ValidatedConfig<InstallConfig>) -> Result<InstallPlan> {
    if !cfg.0.execute {
        return Ok(build_plan(&cfg.0));
    }

    // For execute-mode, callers must arm to perform destructive operations. We only allow a
    // simulated run here (execute=true, dry_run=true).
    if !cfg.0.dry_run {
        anyhow::bail!("execute-mode requires an ArmedConfig; use run_pipeline_execute");
    }

    run_pipeline_impl(cfg.0)
}

fn run_pipeline_impl(cfg: InstallConfig) -> Result<InstallPlan> {
    let plan = build_plan(&cfg);
    if !cfg.execute {
        return Ok(plan);
    }

    let requires_network = !cfg.packages.is_empty() || cfg.download_image || cfg.download_uefi;
    let required_binaries = {
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
    };
    let hal = Arc::new(mash_hal::LinuxHal::new());

    let preflight_cfg = Arc::new(preflight::PreflightConfig::for_install(
        cfg.disk.as_ref().map(PathBuf::from),
        requires_network,
        required_binaries,
    ));
    let mount_cfg = Arc::new(MountStageConfig::from_install_config(&cfg));
    let package_cfg = Arc::new(PackageStageConfig::from_install_config(&cfg));
    let resume_cfg = Arc::new(ResumeStageConfig::from_install_config(&cfg));
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
                let download_cfg = DownloadStageConfig::from_install_config(&cfg);
                StageDefinition {
                    name: stage.name,
                    run: Box::new(move |state, dry_run| {
                        run_download_stage(state, &download_cfg, dry_run)
                    }),
                }
            }
            "Mount plan" => {
                let cfg = Arc::clone(&mount_cfg);
                let hal = Arc::clone(&hal);
                StageDefinition {
                    name: stage.name,
                    run: Box::new(move |state, dry_run| {
                        run_mount_stage(hal.as_ref(), state, &cfg, dry_run)
                    }),
                }
            }
            "Format plan" => {
                let disk_cfg = DiskStageConfig::from_install_config(&cfg);
                let hal = Arc::clone(&hal);
                StageDefinition {
                    name: stage.name,
                    run: Box::new(move |state, dry_run| {
                        run_disk_stage(hal.as_ref(), state, &disk_cfg, dry_run)
                    }),
                }
            }
            "Package plan" => {
                let cfg = Arc::clone(&package_cfg);
                StageDefinition {
                    name: stage.name,
                    run: Box::new(move |state, dry_run| run_package_stage(state, &cfg, dry_run)),
                }
            }
            "Kernel fix check" => {
                let boot_cfg = BootStageConfig::from_install_config(&cfg);
                StageDefinition {
                    name: stage.name,
                    run: Box::new(move |state, dry_run| run_boot_stage(state, &boot_cfg, dry_run)),
                }
            }
            "Resume unit" => {
                let cfg = Arc::clone(&resume_cfg);
                StageDefinition {
                    name: stage.name,
                    run: Box::new(move |state, dry_run| run_resume_stage(state, &cfg, dry_run)),
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

    let runner = StageRunner::new(cfg.state_path.clone(), cfg.dry_run);
    runner.run(&stage_defs)?;

    Ok(plan)
}
