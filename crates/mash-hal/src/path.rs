/// Partition path helper for block devices. Handles nvme/mmcblk postfixing.
pub fn partition_path(disk: &str, num: u32) -> String {
    if disk.contains("nvme") || disk.contains("mmcblk") {
        format!("{}p{}", disk, num)
    } else {
        format!("{}{}", disk, num)
    }
}
