/// Ported from dojo_bundle/usr_local_lib_mash/dojo/menu.sh
use anyhow::Result;
use std::env;
use std::fs;
use std::path::PathBuf;
use std::process::Command;

pub fn run(_args: &[String]) -> Result<()> {
    let state_dir = state_dir();
    fs::create_dir_all(&state_dir)?;
    let completed_flag = state_dir.join("dojo.completed");

    if !has_dialog() {
        println!("⚠️ 'dialog' not installed. Install it with: sudo dnf -y install dialog");
        return Ok(());
    }

    loop {
        let choice = run_dialog();
        match choice.as_deref() {
            Some("1") => {
                let _ = Command::new("sudo")
                    .args(["mash", "dojo", "audio", "--fix"])
                    .status();
            }
            Some("2") => {
                let _ = Command::new("sudo")
                    .args(["mash", "dojo", "graphics", "--apply-dpms-off"])
                    .status();
            }
            Some("3") => {
                let _ = Command::new("sudo")
                    .args(["mash", "dojo", "firewall", "--sane-lan"])
                    .status();
            }
            Some("4") => {
                let _ = Command::new("mash")
                    .args(["dojo", "bootstrap", "--preview-starship"])
                    .status();
            }
            Some("5") => {
                let _ = Command::new("sudo")
                    .args(["mash", "dojo", "bootstrap", "--run"])
                    .status();
            }
            Some("9") => {
                let _ = fs::write(&completed_flag, "");
                let _ = Command::new("clear").status();
                println!("✅ Dojo completed. See you in the dojo, captain. 🥋");
                return Ok(());
            }
            _ => {
                let _ = Command::new("clear").status();
                return Ok(());
            }
        }
    }
}

fn has_dialog() -> bool {
    Command::new("sh")
        .args(["-lc", "command -v dialog >/dev/null 2>&1"])
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

fn run_dialog() -> Option<String> {
    let cmd = "dialog --clear --no-shadow --title 'MASH Dojo 🥋' \
      --menu 'Choose your move:' 18 72 10 \
      1 '🔊 Fix audio (PipeWire/ALSA sanity)' \
      2 '🖥️  Disable DPMS + screensaver (no blackouts)' \
      3 '🛡️  Firewall sane (LAN SSH/Mosh allowed)' \
      4 '⭐ Preview Starship theme' \
      5 '🔥 Run MASH bootstrap (packages, extras)' \
      9 '✅ Exit & don\'t show again' \
      3>&1 1>&2 2>&3";

    let output = Command::new("sh").args(["-lc", cmd]).output().ok()?;
    if !output.status.success() {
        return None;
    }
    let choice = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if choice.is_empty() {
        None
    } else {
        Some(choice)
    }
}

fn state_dir() -> PathBuf {
    if let Ok(xdg) = env::var("XDG_STATE_HOME") {
        if !xdg.is_empty() {
            return PathBuf::from(xdg).join("mash");
        }
    }
    if let Ok(home) = env::var("HOME") {
        if !home.is_empty() {
            return PathBuf::from(home).join(".local/state/mash");
        }
    }
    PathBuf::from("/tmp/mash")
}
