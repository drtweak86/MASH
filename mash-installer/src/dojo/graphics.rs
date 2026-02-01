/// Ported from dojo_bundle/usr_local_lib_mash/dojo/graphics.sh
use anyhow::{anyhow, Result};
use std::env;
use std::process::Command;

pub fn run(args: &[String]) -> Result<()> {
    match args.first().map(String::as_str) {
        Some("--apply-dpms-off") => apply_dpms_off(),
        None | Some("--help") | Some("-h") => Ok(()),
        Some(other) => Err(anyhow!("Unknown arg: {other}")),
    }
}

fn apply_dpms_off() -> Result<()> {
    banner("Disable DPMS + screensaver 🛑😴");

    let has_kwrite = Command::new("sh")
        .args(["-lc", "command -v kwriteconfig5 >/dev/null 2>&1"])
        .status()
        .map(|s| s.success())
        .unwrap_or(false);
    if has_kwrite {
        let _ = Command::new("kwriteconfig5")
            .args([
                "--file",
                "kscreenlockerrc",
                "--group",
                "Daemon",
                "--key",
                "Autolock",
                "false",
            ])
            .status();
        let _ = Command::new("kwriteconfig5")
            .args([
                "--file",
                "powermanagementprofilesrc",
                "--group",
                "AC",
                "--group",
                "DPMSControl",
                "--key",
                "idleTime",
                "0",
            ])
            .status();
        let _ = Command::new("kwriteconfig5")
            .args([
                "--file",
                "powermanagementprofilesrc",
                "--group",
                "AC",
                "--group",
                "DimDisplay",
                "--key",
                "idleTime",
                "0",
            ])
            .status();
        let _ = Command::new("kwriteconfig5")
            .args([
                "--file",
                "powermanagementprofilesrc",
                "--group",
                "AC",
                "--group",
                "SuspendSession",
                "--key",
                "idleTime",
                "0",
            ])
            .status();
    }

    let display = env::var("DISPLAY").unwrap_or_default();
    if !display.is_empty() {
        let _ = Command::new("xset").args(["s", "off"]).status();
        let _ = Command::new("xset").args(["-dpms"]).status();
    }

    let has_qdbus = Command::new("sh")
        .args(["-lc", "command -v qdbus >/dev/null 2>&1"])
        .status()
        .map(|s| s.success())
        .unwrap_or(false);
    if has_qdbus {
        let _ = Command::new("qdbus")
            .args([
                "org.freedesktop.ScreenSaver",
                "/ScreenSaver",
                "SetActive",
                "false",
            ])
            .status();
    }

    println!("✅ DPMS/screensaver tweaks applied (best-effort).");
    println!("Tip: may require logout/login for some KDE power settings.");
    Ok(())
}

fn banner(msg: &str) {
    println!("\n========================================\n{msg}\n========================================");
}
