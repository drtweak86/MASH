use super::config::InstallConfig;
use std::fmt;

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
            name: "Format plan",
            description: format!("{} format operations", format_count),
        },
        StagePlan {
            name: "Mount plan",
            description: format!("{} mount operations", mount_count),
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
        StagePlan {
            name: "Resume unit",
            description: "Install resume unit + request reboot (if required)".to_string(),
        },
    ];

    InstallPlan {
        stages,
        reboot_count: cfg.reboot_count,
    }
}

impl fmt::Display for InstallPlan {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for line in self.summary_lines() {
            writeln!(f, "{}", line)?;
        }
        Ok(())
    }
}
