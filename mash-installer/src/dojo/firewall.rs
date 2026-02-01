/// Ported from dojo_bundle/usr_local_lib_mash/dojo/firewall.sh
use anyhow::{anyhow, Result};
use std::process::Command;

pub fn run(args: &[String]) -> Result<()> {
    match args.first().map(String::as_str) {
        Some("--sane-lan") => sane_lan(),
        None | Some("--help") | Some("-h") => Ok(()),
        Some(other) => Err(anyhow!("Unknown arg: {other}")),
    }
}

fn sane_lan() -> Result<()> {
    println!("== Firewalld sane LAN rules 🛡️ ==");

    let has_firewalld = Command::new("sh")
        .args([
            "-lc",
            "systemctl list-unit-files | grep -q '^firewalld\\.service'",
        ])
        .status()
        .map(|s| s.success())
        .unwrap_or(false);
    if !has_firewalld {
        println!("firewalld not present. Installing...");
        let _ = Command::new("sudo")
            .args(["dnf", "-y", "install", "firewalld"])
            .status();
    }

    let _ = Command::new("sudo")
        .args(["systemctl", "enable", "--now", "firewalld"])
        .status();

    let zone = pick_zone();
    if !zone.is_empty() {
        let _ = Command::new("sudo")
            .args([
                "firewall-cmd",
                "--permanent",
                "--zone",
                &zone,
                "--add-service=ssh",
            ])
            .status();
        let _ = Command::new("sudo")
            .args([
                "firewall-cmd",
                "--permanent",
                "--zone",
                &zone,
                "--add-port=60000-61000/udp",
            ])
            .status();
        let _ = Command::new("sudo")
            .args(["firewall-cmd", "--reload"])
            .status();

        println!("✅ Allowed: ssh + mosh in zone '{zone}'");
    }

    Ok(())
}

fn pick_zone() -> String {
    let zones = Command::new("sh")
        .args(["-lc", "sudo firewall-cmd --get-zones"])
        .output()
        .ok()
        .and_then(|out| String::from_utf8(out.stdout).ok())
        .unwrap_or_default();
    let zones: Vec<&str> = zones.split_whitespace().collect();
    if zones.contains(&"home") {
        return "home".to_string();
    }
    if zones.contains(&"trusted") {
        return "trusted".to_string();
    }
    Command::new("sudo")
        .args(["firewall-cmd", "--get-default-zone"])
        .output()
        .ok()
        .and_then(|out| String::from_utf8(out.stdout).ok())
        .map(|s| s.trim().to_string())
        .unwrap_or_default()
}
