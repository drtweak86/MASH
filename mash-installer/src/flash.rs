use std::process::Command;

pub fn run(image: &str, disk: &str, _uefi: &str, dry_run: bool) -> anyhow::Result<()> {
    println!("💾 Phase 1B – Disk Flash");
    if dry_run {
        println!("(dry-run) would dd {} to {}", image, disk);
        return Ok(());
    }
    let status = Command::new("sudo")
        .args(["dd", &format!("if={}", image), &format!("of={}", disk), "bs=4M", "status=progress"])
        .status()?;
    if !status.success() { anyhow::bail!("dd failed"); }
    Ok(())
}
