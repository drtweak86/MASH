use anyhow::Result;
use std::process::Command;

pub fn run(args: &[String]) -> Result<()> {
    let user = args.first().map(String::as_str).unwrap_or("DrTweak");
    println!("[*] Snapper init for / (btrfs)");

    let mut dnf = Command::new("dnf");
    dnf.args(["install", "-y", "snapper"]);
    run_command_ignore_failure(&mut dnf)?;

    let mut snapper = Command::new("snapper");
    snapper.args(["-c", "root", "create-config", "/"]);
    run_command_ignore_failure(&mut snapper)?;

    let mut chmod = Command::new("chmod");
    chmod.args(["a+rx", "/.snapshots"]);
    run_command_ignore_failure(&mut chmod)?;

    let mut chown = Command::new("chown");
    let group = format!(":{user}");
    chown.args([group.as_str(), "/.snapshots"]);
    run_command_ignore_failure(&mut chown)?;

    Ok(())
}

fn run_command_ignore_failure(cmd: &mut Command) -> Result<()> {
    let status = cmd.status()?;
    if !status.success() {
        log::warn!("command exited with status {status}");
    }
    Ok(())
}
