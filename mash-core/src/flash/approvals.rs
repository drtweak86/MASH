//! Partition approval scaffolding (design stubs, no behavior change).
use crate::{cli::PartitionScheme, flash::config::FlashConfig};
use anyhow::Result;
use mash_hal::PartedOp;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PartitionApprovalMode {
    Global,
    PerPartition,
    PerOp,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApprovalState {
    Pending,
    Approved,
    Denied,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApprovalRequest {
    pub op_id: usize,
    pub disk: String,
    pub partition: Option<u32>,
    pub op_kind: String,
    pub details: String,
}

/// Generate a deterministic list of partition operations for the given config.
/// Behavior: descriptive only; does not execute or mutate state.
pub fn generate_plan_summary(cfg: &FlashConfig) -> Vec<ApprovalRequest> {
    let ops = plan_ops(cfg);
    ops.iter()
        .enumerate()
        .map(|(idx, op)| match op {
            PartedOp::MkLabel { label } => ApprovalRequest {
                op_id: idx,
                disk: cfg.disk.clone(),
                partition: None,
                op_kind: "mklabel".to_string(),
                details: label.clone(),
            },
            PartedOp::MkPart {
                part_type,
                fs_type,
                start,
                end,
            } => ApprovalRequest {
                op_id: idx,
                disk: cfg.disk.clone(),
                partition: None,
                op_kind: "mkpart".to_string(),
                details: format!(
                    "{} {} {} -> {}",
                    part_type.clone(),
                    fs_type.clone(),
                    start,
                    end
                ),
            },
            PartedOp::SetFlag {
                part_num,
                flag,
                state,
            } => ApprovalRequest {
                op_id: idx,
                disk: cfg.disk.clone(),
                partition: Some(*part_num),
                op_kind: "setflag".to_string(),
                details: format!("{} {}", flag, state),
            },
            PartedOp::Print => ApprovalRequest {
                op_id: idx,
                disk: cfg.disk.clone(),
                partition: None,
                op_kind: "print".to_string(),
                details: "print".to_string(),
            },
        })
        .collect()
}

/// Apply approvals to a planned set of ops. Currently a no-op passthrough to preserve behavior.
pub fn apply_approvals(plan: Vec<PartedOp>, _approvals: &[ApprovalState]) -> Result<Vec<PartedOp>> {
    Ok(plan)
}

fn plan_ops(cfg: &FlashConfig) -> Vec<PartedOp> {
    match cfg.scheme {
        PartitionScheme::Mbr => plan_mbr(cfg),
        PartitionScheme::Gpt => plan_gpt(cfg),
    }
}

fn plan_mbr(cfg: &FlashConfig) -> Vec<PartedOp> {
    let boot_end = format!(
        "{}MiB",
        parse_size_to_mib(&cfg.efi_size).unwrap_or(0)
            + parse_size_to_mib(&cfg.boot_size).unwrap_or(0)
    );
    vec![
        PartedOp::MkLabel {
            label: "msdos".to_string(),
        },
        PartedOp::MkPart {
            part_type: "primary".to_string(),
            fs_type: "fat32".to_string(),
            start: "4MiB".to_string(),
            end: cfg.efi_size.clone(),
        },
        PartedOp::SetFlag {
            part_num: 1,
            flag: "boot".to_string(),
            state: "on".to_string(),
        },
        PartedOp::MkPart {
            part_type: "primary".to_string(),
            fs_type: "ext4".to_string(),
            start: cfg.efi_size.clone(),
            end: boot_end.clone(),
        },
        PartedOp::MkPart {
            part_type: "primary".to_string(),
            fs_type: "btrfs".to_string(),
            start: boot_end.clone(),
            end: cfg.root_end.clone(),
        },
        PartedOp::MkPart {
            part_type: "primary".to_string(),
            fs_type: "btrfs".to_string(),
            start: cfg.root_end.clone(),
            end: "100%".to_string(),
        },
        PartedOp::Print,
    ]
}

fn plan_gpt(cfg: &FlashConfig) -> Vec<PartedOp> {
    let boot_end = format!(
        "{}MiB",
        parse_size_to_mib(&cfg.efi_size).unwrap_or(0)
            + parse_size_to_mib(&cfg.boot_size).unwrap_or(0)
    );
    vec![
        PartedOp::MkLabel {
            label: "gpt".to_string(),
        },
        PartedOp::MkPart {
            part_type: "primary".to_string(),
            fs_type: "fat32".to_string(),
            start: "4MiB".to_string(),
            end: cfg.efi_size.clone(),
        },
        PartedOp::SetFlag {
            part_num: 1,
            flag: "esp".to_string(),
            state: "on".to_string(),
        },
        PartedOp::MkPart {
            part_type: "primary".to_string(),
            fs_type: "ext4".to_string(),
            start: cfg.efi_size.clone(),
            end: boot_end.clone(),
        },
        PartedOp::MkPart {
            part_type: "primary".to_string(),
            fs_type: "btrfs".to_string(),
            start: boot_end.clone(),
            end: cfg.root_end.clone(),
        },
        PartedOp::MkPart {
            part_type: "primary".to_string(),
            fs_type: "btrfs".to_string(),
            start: cfg.root_end.clone(),
            end: "100%".to_string(),
        },
        PartedOp::Print,
    ]
}

fn parse_size_to_mib(raw: &str) -> Option<u64> {
    let trimmed = raw.trim().to_ascii_lowercase();
    let (num_str, suffix) = if let Some(rest) = trimmed.strip_suffix("mib") {
        (rest, "mib")
    } else if let Some(rest) = trimmed.strip_suffix("mb") {
        (rest, "mb")
    } else if let Some(rest) = trimmed.strip_suffix('m') {
        (rest, "m")
    } else if let Some(rest) = trimmed.strip_suffix("gib") {
        (rest, "gib")
    } else if let Some(rest) = trimmed.strip_suffix("gb") {
        (rest, "gb")
    } else if let Some(rest) = trimmed.strip_suffix('g') {
        (rest, "g")
    } else {
        return None;
    };
    let value: u64 = num_str.trim().parse().ok()?;
    let mib = match suffix {
        "mib" | "mb" | "m" => value,
        "gib" | "gb" | "g" => value.saturating_mul(1024),
        _ => return None,
    };
    Some(mib)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::PartitionScheme;
    use crate::flash::config::FlashConfig;
    use std::path::PathBuf;

    fn base_config(scheme: PartitionScheme) -> FlashConfig {
        FlashConfig {
            os_distro: Some("Fedora".to_string()),
            os_flavour: Some("KDE".to_string()),
            disk_identity: None,
            efi_source: Some("local".to_string()),
            image: PathBuf::from("/tmp/image.raw"),
            disk: "/dev/sdx".to_string(),
            scheme,
            uefi_dir: PathBuf::from("/tmp/uefi"),
            dry_run: true,
            auto_unmount: true,
            locale: None,
            early_ssh: true,
            progress_tx: None,
            efi_size: "1024MiB".to_string(),
            boot_size: "2048MiB".to_string(),
            root_end: "1800GiB".to_string(),
            partition_approval_mode: PartitionApprovalMode::Global,
        }
    }

    #[test]
    fn summary_matches_mbr_plan_length() {
        let cfg = base_config(PartitionScheme::Mbr);
        let summary = generate_plan_summary(&cfg);
        assert_eq!(summary.len(), 7);
        assert!(summary.iter().any(|r| r.op_kind == "mklabel"));
    }

    #[test]
    fn summary_matches_gpt_plan_length() {
        let cfg = base_config(PartitionScheme::Gpt);
        let summary = generate_plan_summary(&cfg);
        assert_eq!(summary.len(), 7);
        assert!(summary.iter().any(|r| r.op_kind == "mklabel"));
    }

    #[test]
    fn apply_approvals_passthrough_global() {
        let cfg = base_config(PartitionScheme::Gpt);
        let plan = super::plan_ops(&cfg);
        let result = apply_approvals(plan.clone(), &[]);
        assert!(result.is_ok());
        assert_eq!(result.unwrap().len(), plan.len());
    }

    #[test]
    fn parse_sizes_accepts_units() {
        assert_eq!(parse_size_to_mib("1GiB"), Some(1024));
        assert_eq!(parse_size_to_mib("512MiB"), Some(512));
        assert!(parse_size_to_mib("bad").is_none());
    }
}
