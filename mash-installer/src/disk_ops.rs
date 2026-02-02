use anyhow::Result;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiskInfo {
    pub id: String,
    pub size_bytes: u64,
    pub model: String,
}

impl DiskInfo {
    pub fn new(id: String, size_bytes: u64, model: String) -> Self {
        Self {
            id,
            size_bytes,
            model,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PartitionPlan {
    pub disk_id: String,
    pub partitions: Vec<PartitionSpec>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PartitionSpec {
    pub size_bytes: u64,
    pub filesystem: FileSystemType,
    pub mount_point: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FileSystemType {
    Fat32,
    Ext4,
    Btrfs,
    Xfs,
}

pub fn probe_disks(dry_run: bool) -> Result<Vec<DiskInfo>> {
    if dry_run {
        log::info!("DRY RUN: Simulating disk probing.");
        return Ok(vec![
            DiskInfo::new(
                "/dev/sda".to_string(),
                1_000_000_000_000,
                "ExampleDiskA".to_string(),
            ),
            DiskInfo::new(
                "/dev/sdb".to_string(),
                500_000_000_000,
                "ExampleDiskB".to_string(),
            ),
        ]);
    }

    unimplemented!("Real disk probing is not implemented yet");
}

pub fn plan_partitioning(disk: &DiskInfo, dry_run: bool) -> Result<PartitionPlan> {
    if dry_run {
        log::info!("DRY RUN: Proposing partition scheme for {}.", disk.id);
        let partitions = vec![
            PartitionSpec {
                size_bytes: 512 * 1024 * 1024,
                filesystem: FileSystemType::Fat32,
                mount_point: Some("/boot/efi".to_string()),
            },
            PartitionSpec {
                size_bytes: 1024 * 1024 * 1024,
                filesystem: FileSystemType::Ext4,
                mount_point: Some("/boot".to_string()),
            },
            PartitionSpec {
                size_bytes: 20 * 1024 * 1024 * 1024,
                filesystem: FileSystemType::Ext4,
                mount_point: Some("/".to_string()),
            },
            PartitionSpec {
                size_bytes: 0,
                filesystem: FileSystemType::Ext4,
                mount_point: Some("/data".to_string()),
            },
        ];
        return Ok(PartitionPlan {
            disk_id: disk.id.clone(),
            partitions,
        });
    }

    unimplemented!("Real partition planning is not implemented yet");
}

pub fn format_partitions(plan: &PartitionPlan, dry_run: bool) -> Result<()> {
    if dry_run {
        for partition in &plan.partitions {
            log::info!(
                "DRY RUN: Formatting partition with {:?} filesystem.",
                partition.filesystem
            );
        }
        return Ok(());
    }

    unimplemented!("Real partition formatting is not implemented yet");
}

pub fn mount_partitions(plan: &PartitionPlan, dry_run: bool) -> Result<()> {
    if dry_run {
        for partition in &plan.partitions {
            if let Some(mount_point) = partition.mount_point.as_ref() {
                log::info!(
                    "DRY RUN: Mounting {:?} to {:?}.",
                    partition.filesystem,
                    mount_point
                );
            }
        }
        return Ok(());
    }

    unimplemented!("Real partition mounting is not implemented yet");
}

pub fn verify_disk_operations(plan: &PartitionPlan, dry_run: bool) -> Result<()> {
    if dry_run {
        log::info!("DRY RUN: Verifying disk operations for {:?}.", plan.disk_id);
        return Ok(());
    }

    unimplemented!("Real disk verification is not implemented yet");
}
