use anyhow::Result;
use std::process::Command;

pub fn run(args: &[String]) -> Result<()> {
    let user = args.first().map(String::as_str).unwrap_or("DrTweak");
    println!("[*] KDE screensaver/DPMS nuke (best effort)");

    let mut lock = Command::new("sudo");
    lock.args([
        "-u",
        user,
        "sh",
        "-c",
        "kwriteconfig5 --file kscreenlockerrc --group Daemon --key Autolock false",
    ]);
    run_command_ignore_failure(&mut lock)?;

    let mut suspend = Command::new("sudo");
    suspend.args([
        "-u",
        user,
        "sh",
        "-c",
        "kwriteconfig5 --file powerdevilrc --group AC --group SuspendSession --key suspendType 0",
    ]);
    run_command_ignore_failure(&mut suspend)?;

    let mut xset_off = Command::new("sudo");
    xset_off.args(["-u", user, "sh", "-c", "xset s off"]);
    run_command_ignore_failure(&mut xset_off)?;

    let mut xset_dpms = Command::new("sudo");
    xset_dpms.args(["-u", user, "sh", "-c", "xset -dpms"]);
    run_command_ignore_failure(&mut xset_dpms)?;

    Ok(())
}

fn run_command_ignore_failure(cmd: &mut Command) -> Result<()> {
    let status = cmd.status()?;
    if !status.success() {
        log::warn!("command exited with status {status}");
    }
    Ok(())
}
