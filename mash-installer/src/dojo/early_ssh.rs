use anyhow::Result;
use std::fs;
use std::io::Write;
use std::process::Command;

pub fn run(_args: &[String]) -> Result<()> {
    let log_dir = "/data/mash-logs";
    fs::create_dir_all(log_dir)?;
    let log_path = format!("{log_dir}/early-ssh.log");
    let mut log = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_path)?;

    let timestamp = Command::new("date")
        .args(["-Is"])
        .output()
        .ok()
        .and_then(|out| String::from_utf8(out.stdout).ok())
        .unwrap_or_else(|| "unknown".to_string());
    let timestamp = timestamp.trim();

    writeln!(log, "=== {timestamp} :: early-ssh start ===")?;
    let hostname = Command::new("hostname")
        .output()
        .ok()
        .and_then(|out| String::from_utf8(out.stdout).ok())
        .unwrap_or_else(|| "unknown".to_string());
    writeln!(log, "hostname: {}", hostname.trim())?;

    let has_sshd = Command::new("sh")
        .args([
            "-lc",
            "systemctl list-unit-files | grep -q '^sshd\\.service'",
        ])
        .status()
        .map(|s| s.success())
        .unwrap_or(false);
    if has_sshd {
        let _ = Command::new("systemctl")
            .args(["enable", "--now", "sshd"])
            .status();
        writeln!(log, "✅ sshd enabled")?;
    } else {
        writeln!(log, "⚠️ sshd.service not present")?;
    }

    let has_avahi = Command::new("sh")
        .args([
            "-lc",
            "systemctl list-unit-files | grep -q '^avahi-daemon\\.service'",
        ])
        .status()
        .map(|s| s.success())
        .unwrap_or(false);
    if has_avahi {
        let _ = Command::new("systemctl")
            .args(["enable", "--now", "avahi-daemon"])
            .status();
        writeln!(log, "✅ avahi-daemon enabled (mDNS)")?;
    } else {
        writeln!(log, "ℹ️ avahi-daemon not installed yet")?;
    }

    let has_firewalld = Command::new("sh")
        .args([
            "-lc",
            "systemctl list-unit-files | grep -q '^firewalld\\.service'",
        ])
        .status()
        .map(|s| s.success())
        .unwrap_or(false);
    if has_firewalld {
        let _ = Command::new("systemctl")
            .args(["enable", "--now", "firewalld"])
            .status();

        let zone = pick_firewalld_zone().unwrap_or_default();
        if !zone.is_empty() {
            let _ = Command::new("firewall-cmd")
                .args(["--permanent", "--zone", &zone, "--add-service=ssh"])
                .status();

            let has_mosh = Command::new("sh")
                .args([
                    "-lc",
                    "firewall-cmd --get-services | tr ' ' '\n' | grep -qx mosh",
                ])
                .status()
                .map(|s| s.success())
                .unwrap_or(false);
            if has_mosh {
                let _ = Command::new("firewall-cmd")
                    .args(["--permanent", "--zone", &zone, "--add-service=mosh"])
                    .status();
            } else {
                let _ = Command::new("firewall-cmd")
                    .args(["--permanent", "--zone", &zone, "--add-port=60000-61000/udp"])
                    .status();
            }
            let _ = Command::new("firewall-cmd").args(["--reload"]).status();
            writeln!(log, "✅ firewalld: opened SSH + mosh on zone={zone}")?;
        }
    } else {
        writeln!(log, "ℹ️ firewalld not present")?;
    }

    let timestamp_end = Command::new("date")
        .args(["-Is"])
        .output()
        .ok()
        .and_then(|out| String::from_utf8(out.stdout).ok())
        .unwrap_or_else(|| "unknown".to_string());
    writeln!(log, "=== {} :: early-ssh done ===", timestamp_end.trim())?;

    Ok(())
}

fn pick_firewalld_zone() -> Option<String> {
    let zones = Command::new("sh")
        .args(["-lc", "firewall-cmd --get-zones"])
        .output()
        .ok()
        .and_then(|out| String::from_utf8(out.stdout).ok())?;
    let zones = zones
        .split_whitespace()
        .map(|s| s.trim().to_string())
        .collect::<Vec<_>>();
    if zones.iter().any(|z| z == "home") {
        return Some("home".to_string());
    }
    if zones.iter().any(|z| z == "trusted") {
        return Some("trusted".to_string());
    }
    let default_zone = Command::new("firewall-cmd")
        .args(["--get-default-zone"])
        .output()
        .ok()
        .and_then(|out| String::from_utf8(out.stdout).ok())
        .map(|s| s.trim().to_string())?;
    if default_zone.is_empty() {
        None
    } else {
        Some(default_zone)
    }
}
