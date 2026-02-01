/// Ported from dojo_bundle/usr_local_lib_mash/dojo/rclone.sh
use anyhow::Result;
use std::process::Command;

pub fn run(_args: &[String]) -> Result<()> {
    banner("rclone: install + config");

    let _ = Command::new("sudo")
        .args(["dnf", "install", "-y", "rclone"])
        .status();

    println!(
        "\nRun:\n  rclone config\n\nThen test:\n  rclone lsd <remote>:\n\nWhen you're ready, we can add:\n  - systemd timers\n  - encrypted remotes\n  - bandwidth schedules\n"
    );

    Ok(())
}

fn banner(msg: &str) {
    println!("==============================================================================");
    println!("{msg}");
    println!("==============================================================================");
}
