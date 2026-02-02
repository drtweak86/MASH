use crate::stage_runner::{StageDefinition, StageRunner};
use crate::system_config::packages::PackageManager;
use crate::{boot_config, disk_ops, system_config};
use anyhow::Result;
use std::fmt;
use std::path::PathBuf;

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

    let stages = vec![
        StagePlan {
            name: "Preflight",
            description: "Read-only checks".to_string(),
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

pub fn run_pipeline(cfg: &InstallConfig) -> Result<InstallPlan> {
    let plan = build_plan(cfg);
    if cfg.execute && !cfg.confirmed {
        anyhow::bail!("Execution requires explicit confirmation");
    }
    let stage_defs = plan
        .stages
        .clone()
        .into_iter()
        .map(|stage| {
            let name = stage.name;
            StageDefinition {
                name,
                run: Box::new(move |dry_run| {
                    if dry_run {
                        log::info!("DRY RUN: {}", name);
                    } else {
                        log::info!("Stage: {}", name);
                    }
                    Ok(())
                }),
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
        };
        let plan = build_plan(&cfg);
        assert_eq!(plan.stages.len(), 6);
        assert_eq!(plan.stages[0].name, "Preflight");
        assert_eq!(plan.stages[5].name, "Kernel fix check");
    }
}
