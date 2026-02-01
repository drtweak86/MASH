use anyhow::Result;
use std::process::Command;

pub fn run(args: &[String]) -> Result<()> {
    let user = args.first().map(String::as_str).unwrap_or("DrTweak");
    println!("[*] Snapper init for / (btrfs)");

    let _ = Command::new("dnf")
        .args(["install", "-y", "snapper"])
        .status();
    let _ = Command::new("snapper")
        .args(["-c", "root", "create-config", "/"])
        .status();
    let _ = Command::new("chmod").args(["a+rx", "/.snapshots"]).status();
    let _ = Command::new("chown")
        .args([&format!(":{user}"), "/.snapshots"])
        .status();

    Ok(())
}
