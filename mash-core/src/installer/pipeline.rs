use crate::stage_runner::{StageDefinition, StageRunner};
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
                "USB-root alignment enabled".to_string()
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

    let runner = StageRunner::new(cfg.state_path.clone(), cfg.dry_run);
    let _ = runner.run(&stage_defs)?;
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
            reboot_count: 1,
        };
        let plan = build_plan(&cfg);
        assert_eq!(plan.stages.len(), 6);
        assert_eq!(plan.stages[0].name, "Preflight");
        assert_eq!(plan.stages[5].name, "Kernel fix check");
    }
}
