use anyhow::Result;
use std::process::Command;

pub fn run(_args: &[String]) -> Result<()> {
    println!("[*] Locale: en_GB + GB keymap");

    let _ = Command::new("dnf")
        .args(["install", "-y", "langpacks-en_GB"])
        .status();
    let _ = Command::new("localectl")
        .args(["set-locale", "LANG=en_GB.UTF-8"])
        .status();
    let _ = Command::new("localectl")
        .args(["set-x11-keymap", "gb"])
        .status();

    Ok(())
}
