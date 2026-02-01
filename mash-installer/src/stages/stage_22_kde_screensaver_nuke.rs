use anyhow::Result;
use std::process::Command;

pub fn run(args: &[String]) -> Result<()> {
    let user = args.first().map(String::as_str).unwrap_or("DrTweak");
    println!("[*] KDE screensaver/DPMS nuke (best effort)");

    let _ = Command::new("sudo")
        .args([
            "-u",
            user,
            "sh",
            "-c",
            "kwriteconfig5 --file kscreenlockerrc --group Daemon --key Autolock false",
        ])
        .status();
    let _ = Command::new("sudo")
        .args([
            "-u",
            user,
            "sh",
            "-c",
            "kwriteconfig5 --file powerdevilrc --group AC --group SuspendSession --key suspendType 0",
        ])
        .status();
    let _ = Command::new("sudo")
        .args(["-u", user, "sh", "-c", "xset s off"])
        .status();
    let _ = Command::new("sudo")
        .args(["-u", user, "sh", "-c", "xset -dpms"])
        .status();

    Ok(())
}
