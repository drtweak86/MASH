use mash_installer::disk_ops;

#[test]
fn probe_disks_dry_run_returns_examples() {
    let disks = disk_ops::probe_disks(true).expect("dry run probe should succeed");

    let expected = vec![
        disk_ops::DiskInfo::new(
            "/dev/sda".to_string(),
            1_000_000_000_000,
            "ExampleDiskA".to_string(),
        ),
        disk_ops::DiskInfo::new(
            "/dev/sdb".to_string(),
            500_000_000_000,
            "ExampleDiskB".to_string(),
        ),
    ];

    assert_eq!(disks, expected);
}

#[test]
#[should_panic(expected = "Real disk probing is not implemented yet")]
fn probe_disks_non_dry_run_panics() {
    let _ = disk_ops::probe_disks(false);
}

#[test]
fn plan_partitioning_dry_run_returns_plan() {
    let disk = disk_ops::DiskInfo::new(
        "/dev/sdz".to_string(),
        2_000_000_000_000,
        "ExampleDiskZ".to_string(),
    );

    let plan = disk_ops::plan_partitioning(&disk, true).expect("dry run plan should succeed");

    assert_eq!(plan.disk_id, disk.id);
    assert_eq!(plan.partitions.len(), 4);
    assert_eq!(
        plan.partitions[0].filesystem,
        disk_ops::FileSystemType::Fat32
    );
    assert_eq!(plan.partitions[0].mount_point.as_deref(), Some("/boot/efi"));
}

#[test]
#[should_panic(expected = "Real partition planning is not implemented yet")]
fn plan_partitioning_non_dry_run_panics() {
    let disk = disk_ops::DiskInfo::new(
        "/dev/sdz".to_string(),
        2_000_000_000_000,
        "ExampleDiskZ".to_string(),
    );

    let _ = disk_ops::plan_partitioning(&disk, false);
}

#[test]
fn format_partitions_dry_run_succeeds() {
    let plan = disk_ops::PartitionPlan {
        disk_id: "/dev/sdz".to_string(),
        partitions: vec![
            disk_ops::PartitionSpec {
                size_bytes: 512 * 1024 * 1024,
                filesystem: disk_ops::FileSystemType::Fat32,
                mount_point: Some("/boot/efi".to_string()),
            },
            disk_ops::PartitionSpec {
                size_bytes: 0,
                filesystem: disk_ops::FileSystemType::Ext4,
                mount_point: Some("/".to_string()),
            },
        ],
    };

    disk_ops::format_partitions(&plan, true).expect("dry run format should succeed");
}

#[test]
#[should_panic(expected = "Real partition formatting is not implemented yet")]
fn format_partitions_non_dry_run_panics() {
    let plan = disk_ops::PartitionPlan {
        disk_id: "/dev/sdz".to_string(),
        partitions: vec![],
    };

    let _ = disk_ops::format_partitions(&plan, false);
}
