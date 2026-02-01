/// Ported from dojo_bundle/usr_local_lib_mash/dojo/audio.sh
use anyhow::{anyhow, Result};
use std::process::Command;

pub fn run(args: &[String]) -> Result<()> {
    match args.first().map(String::as_str) {
        Some("--fix") => fix_audio(),
        None | Some("--help") | Some("-h") => Ok(()),
        Some(other) => Err(anyhow!("Unknown arg: {other}")),
    }
}

fn fix_audio() -> Result<()> {
    println!("== Audio sanity 🔊 ==");
    let _ = Command::new("sudo")
        .args([
            "dnf",
            "-y",
            "install",
            "alsa-utils",
            "pipewire",
            "wireplumber",
        ])
        .status();

    let _ = Command::new("systemctl")
        .args(["--user", "enable", "--now", "pipewire", "wireplumber"])
        .status();
    let _ = Command::new("systemctl")
        .args(["--user", "restart", "pipewire", "wireplumber"])
        .status();

    println!("Devices:");
    let _ = Command::new("aplay").args(["-l"]).status();
    let _ = Command::new("wpctl").args(["status"]).status();
    println!("✅ Audio fix attempt complete.");
    Ok(())
}
