use anyhow::Result;
use std::fs;
use std::io::{self, Write};
use std::process::Command;

pub fn run(args: &[String]) -> Result<()> {
    let user_name = args.first().map(String::as_str).unwrap_or("drtweak");
    let log_dir = "/data/mash-logs";
    let log_file = format!("{log_dir}/brave.log");
    fs::create_dir_all(log_dir)?;

    let mut log = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_file)?;

    let mut log_line = |line: &str| -> io::Result<()> {
        let timestamp = Command::new("date")
            .args(["-Is"])
            .output()
            .ok()
            .and_then(|out| String::from_utf8(out.stdout).ok())
            .unwrap_or_else(|| "unknown".to_string());
        let timestamp = timestamp.trim();
        writeln!(log, "[{timestamp}] {line}")?;
        println!("[{timestamp}] {line}");
        Ok(())
    };

    log_line("=== Brave install + default browser ===")?;

    let online = Command::new("curl")
        .args(["-fsSL", "--max-time", "8", "https://www.google.com"])
        .status()
        .map(|s| s.success())
        .unwrap_or(false);
    if !online {
        log_line("No internet detected; skipping Brave install for now.")?;
        return Ok(());
    }

    log_line("Installing Brave repo...")?;
    let _ = Command::new("rpm")
        .args([
            "--import",
            "https://brave-browser-rpm-release.s3.brave.com/brave-core.asc",
        ])
        .status();

    let repo_file = "/etc/yum.repos.d/brave-browser.repo";
    let _ = Command::new("curl")
        .args([
            "-fsSL",
            "https://brave-browser-rpm-release.s3.brave.com/brave-browser.repo",
            "-o",
            repo_file,
        ])
        .status();

    log_line("dnf install brave-browser")?;
    let brave_ok = Command::new("dnf")
        .args(["install", "-y", "brave-browser"])
        .status()
        .map(|s| s.success())
        .unwrap_or(false);
    if !brave_ok {
        log_line("Brave install failed (repo/arch?).")?;
        return Ok(());
    }

    log_line(&format!("Setting default browser for user: {user_name}"))?;
    if user_exists(user_name) {
        set_default_browser(user_name, "brave-browser.desktop");
        patch_mimeapps_list(user_name, "brave-browser.desktop")?;
    } else {
        log_line(&format!(
            "User {user_name} not found yet; default browser will be set later via Dojo."
        ))?;
    }

    log_line("Done.")?;
    Ok(())
}

fn user_exists(user: &str) -> bool {
    Command::new("id")
        .arg(user)
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

fn set_default_browser(user: &str, desktop_id: &str) {
    let _ = Command::new("sudo")
        .args([
            "-u",
            user,
            "xdg-settings",
            "set",
            "default-web-browser",
            desktop_id,
        ])
        .status();
}

fn patch_mimeapps_list(user: &str, desktop_id: &str) -> Result<()> {
    let output = Command::new("getent").args(["passwd", user]).output()?;
    let line = String::from_utf8_lossy(&output.stdout);
    let home = line.split(':').nth(5).unwrap_or("").trim();
    if home.is_empty() {
        return Ok(());
    }

    let config_dir = format!("{home}/.config");
    let mimeapps = format!("{config_dir}/mimeapps.list");
    fs::create_dir_all(&config_dir)?;

    let existing = fs::read_to_string(&mimeapps).unwrap_or_default();
    let mut lines = existing.lines().collect::<Vec<_>>();
    if !lines
        .iter()
        .any(|line| line.trim() == "[Default Applications]")
    {
        lines.push("[Default Applications]");
    }

    let entries = [
        format!("x-scheme-handler/http={desktop_id}"),
        format!("x-scheme-handler/https={desktop_id}"),
    ];

    for entry in &entries {
        if !lines.iter().any(|line| line.trim() == entry) {
            lines.push(entry);
        }
    }

    let mut file = fs::OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(&mimeapps)?;
    for line in lines {
        writeln!(file, "{line}")?;
    }

    let _ = Command::new("chown")
        .args([&format!("{user}:{user}"), &mimeapps])
        .status();

    Ok(())
}
