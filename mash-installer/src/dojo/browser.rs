/// Ported from dojo_bundle/usr_local_lib_mash/dojo/browser.sh
use anyhow::Result;
use std::env;
use std::fs;
use std::io::{self, Write};
use std::process::Command;

pub fn run(args: &[String]) -> Result<()> {
    let mut user = args.first().map(String::as_str).unwrap_or("").to_string();
    if user.is_empty() {
        user = env::var("SUDO_USER").unwrap_or_default();
    }
    if user.is_empty() {
        user = "drtweak".to_string();
    }

    let log_dir = "/data/mash-logs";
    fs::create_dir_all(log_dir)?;
    let log_file = format!("{log_dir}/dojo-browser.log");
    let mut log = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_file)?;

    let mut log_line = |line: &str| -> io::Result<()> {
        let ts = Command::new("date")
            .args(["-Is"])
            .output()
            .ok()
            .and_then(|out| String::from_utf8(out.stdout).ok())
            .unwrap_or_else(|| "unknown".to_string());
        let ts = ts.trim();
        writeln!(log, "[{ts}] {line}")?;
        println!("[{ts}] {line}");
        Ok(())
    };

    println!("\n🌐 Browser dojo: Brave install + set default (user: {user})\n");

    let has_curl = Command::new("sh")
        .args(["-lc", "command -v curl"])
        .status()
        .map(|s| s.success())
        .unwrap_or(false);
    if !has_curl {
        let _ = Command::new("sudo")
            .args(["dnf", "install", "-y", "curl"])
            .status();
    }

    let online = Command::new("curl")
        .args(["-fsSL", "--max-time", "8", "https://www.google.com"])
        .status()
        .map(|s| s.success())
        .unwrap_or(false);
    if !online {
        log_line("No internet detected; cannot install Brave yet.")?;
        println!("❌ No internet yet. Come back after Wi‑Fi is up.");
        return Ok(());
    }

    log_line("Importing Brave key + repo...")?;
    let _ = Command::new("sudo")
        .args([
            "rpm",
            "--import",
            "https://brave-browser-rpm-release.s3.brave.com/brave-core.asc",
        ])
        .status();
    let _ = Command::new("sh")
        .args([
            "-lc",
            "curl -fsSL https://brave-browser-rpm-release.s3.brave.com/brave-browser.repo | sudo tee /etc/yum.repos.d/brave-browser.repo >/dev/null",
        ])
        .status();

    log_line("Installing brave-browser...")?;
    let brave_ok = Command::new("sudo")
        .args(["dnf", "install", "-y", "brave-browser"])
        .status()
        .map(|s| s.success())
        .unwrap_or(false);
    if !brave_ok {
        println!("❌ Brave install failed.");
        return Ok(());
    }

    log_line("Setting default browser...")?;
    let user_exists = Command::new("id")
        .arg(&user)
        .status()
        .map(|s| s.success())
        .unwrap_or(false);
    if user_exists {
        let _ = Command::new("sudo")
            .args([
                "-u",
                &user,
                "xdg-settings",
                "set",
                "default-web-browser",
                "brave-browser.desktop",
            ])
            .status();
        println!("✅ Default browser set to Brave for {user} (best-effort).");
    } else {
        println!("⚠️ User {user} not found; log in once then rerun this option.");
    }

    println!("\nDone. Launch Brave from the app menu.");
    Ok(())
}
