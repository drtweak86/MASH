/// Ported from dojo_bundle/usr_local_lib_mash/dojo/snapper.sh
use anyhow::Result;
use std::process::Command;

pub fn run(_args: &[String]) -> Result<()> {
    banner("Snapper: Atomic Shield for / (Btrfs)");

    let _ = Command::new("sudo")
        .args(["dnf", "install", "-y", "snapper", "btrfs-assistant"])
        .status();

    let _ = Command::new("sudo")
        .args(["snapper", "-c", "root", "create-config", "/"])
        .status();

    let _ = Command::new("sudo")
        .args(["chmod", "a+rx", "/.snapshots"])
        .status();

    println!("\nSnapper ready. GUI: btrfs-assistant");
    println!("CLI: snapper -c root list");

    Ok(())
}

fn banner(msg: &str) {
    println!("==============================================================================");
    println!("{msg}");
    println!("==============================================================================");
}
