use anyhow::Result;
use std::process::Command;

pub fn run(_args: &[String]) -> Result<()> {
    println!("[*] Locale: en_GB + GB keymap");

    let mut dnf = Command::new("dnf");
    dnf.args(["install", "-y", "langpacks-en_GB"]);
    run_command_ignore_failure(&mut dnf)?;

    let mut locale = Command::new("localectl");
    locale.args(["set-locale", "LANG=en_GB.UTF-8"]);
    run_command_ignore_failure(&mut locale)?;

    let mut keymap = Command::new("localectl");
    keymap.args(["set-x11-keymap", "gb"]);
    run_command_ignore_failure(&mut keymap)?;

    Ok(())
}

fn run_command_ignore_failure(cmd: &mut Command) -> Result<()> {
    let status = cmd.status()?;
    if !status.success() {
        log::warn!("command exited with status {status}");
    }
    Ok(())
}
