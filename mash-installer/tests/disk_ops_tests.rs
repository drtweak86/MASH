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
