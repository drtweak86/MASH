use std::error::Error;

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

pub fn probe_disks(dry_run: bool) -> Result<Vec<DiskInfo>, Box<dyn Error>> {
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
