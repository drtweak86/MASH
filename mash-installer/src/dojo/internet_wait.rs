use anyhow::Result;
use std::fs;
use std::io::Write;
use std::process::Command;
use std::thread;
use std::time::Duration;

pub fn run(_args: &[String]) -> Result<()> {
    let log_dir = "/data/mash-logs";
    fs::create_dir_all(log_dir)?;
    let log_path = format!("{log_dir}/internet-wait.log");
    let mut log = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_path)?;

    writeln!(log, "== Internet wait ==")?;
    let _ = Command::new("date").status();

    ply("🌐 Waiting for network…");

    for _ in 0..60 {
        let has_default = Command::new("sh")
            .args(["-lc", "ip route | grep -q '^default '"])
            .status()
            .map(|s| s.success())
            .unwrap_or(false);
        if has_default {
            break;
        }
        thread::sleep(Duration::from_secs(1));
    }

    for _ in 0..60 {
        let dns_ok = Command::new("sh")
            .args([
                "-lc",
                "getent ahosts deb.debian.org >/dev/null 2>&1 || getent ahosts github.com >/dev/null 2>&1",
            ])
            .status()
            .map(|s| s.success())
            .unwrap_or(false);
        if dns_ok {
            ply("✅ Network + DNS OK");
            writeln!(log, "OK: DNS resolution working")?;
            return Ok(());
        }
        thread::sleep(Duration::from_secs(1));
    }

    ply("⚠️ No DNS yet — continuing anyway");
    writeln!(log, "WARN: DNS check failed after timeout; continuing")?;
    Ok(())
}

fn ply(message: &str) {
    let has_ply = Command::new("sh")
        .args([
            "-lc",
            "command -v plymouth >/dev/null 2>&1 && plymouth --ping >/dev/null 2>&1",
        ])
        .status()
        .map(|s| s.success())
        .unwrap_or(false);
    if has_ply {
        let _ = Command::new("plymouth")
            .args(["display-message", &format!("--text={message}")])
            .status();
    }
}
