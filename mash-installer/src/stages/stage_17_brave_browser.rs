use anyhow::Result;
use std::fs;
use std::io::{self, Write};
use std::process::Command;

const REPO_CONTENT: &str = r#"[brave-browser]
name=Brave Browser
baseurl=https://brave-browser-rpm-release.s3.brave.com/x86_64/
enabled=1
gpgcheck=1
gpgkey=https://brave-browser-rpm-release.s3.brave.com/brave-core.asc
"#;

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
        writeln!(log, "{line}")?;
        println!("{line}");
        Ok(())
    };

    log_line("================================================================================")?;
    log_line("🌐 Brave browser + default browser setup")?;
    log_line("================================================================================")?;

    let _ = Command::new("dnf")
        .args([
            "install",
            "-y",
            "--setopt=install_weak_deps=True",
            "dnf-plugins-core",
            "curl",
            "ca-certificates",
        ])
        .status();

    let repo_file = "/etc/yum.repos.d/brave-browser.repo";
    if fs::metadata(repo_file).is_err() {
        log_line(&format!("➕ Adding Brave repo: {repo_file}"))?;
        let _ = Command::new("rpm")
            .args([
                "--import",
                "https://brave-browser-rpm-release.s3.brave.com/brave-core.asc",
            ])
            .status();
        fs::write(repo_file, REPO_CONTENT)?;
    }

    log_line("📦 Installing brave-browser (best-effort; may be unavailable on aarch64)")?;
    let _ = Command::new("dnf")
        .args(["install", "-y", "--skip-unavailable", "brave-browser"])
        .status();

    let brave_installed = Command::new("rpm")
        .args(["-q", "brave-browser"])
        .status()
        .map(|s| s.success())
        .unwrap_or(false);

    if !brave_installed {
        log_line("⚠️ brave-browser not installed (likely no aarch64 build in repo).")?;
        log_line("   Dojo will offer alternatives (Firefox) and you can revisit later.")?;
        let _ = Command::new("dnf")
            .args([
                "install",
                "-y",
                "--setopt=install_weak_deps=True",
                "firefox",
            ])
            .status();

        let firefox_installed = Command::new("rpm")
            .args(["-q", "firefox"])
            .status()
            .map(|s| s.success())
            .unwrap_or(false);

        if firefox_installed {
            log_line("🦊 Falling back to Firefox as default browser.")?;
            if user_exists(user_name) {
                set_default_browser(user_name, "firefox.desktop");
            }
        }

        return Ok(());
    }

    log_line(&format!(
        "🔧 Setting default browser to Brave for user: {user_name}"
    ))?;
    if user_exists(user_name) {
        set_default_browser(user_name, "brave-browser.desktop");
        patch_mimeapps_list(user_name, "brave-browser.desktop")?;
    } else {
        log_line(&format!(
            "⚠️ user '{user_name}' not found yet; skipping default-browser binding."
        ))?;
    }

    log_line("✅ Brave step complete.")?;
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
    let _ = Command::new("sudo")
        .args([
            "-u",
            user,
            "xdg-mime",
            "default",
            desktop_id,
            "x-scheme-handler/http",
        ])
        .status();
    let _ = Command::new("sudo")
        .args([
            "-u",
            user,
            "xdg-mime",
            "default",
            desktop_id,
            "x-scheme-handler/https",
        ])
        .status();
    let _ = Command::new("sudo")
        .args(["-u", user, "xdg-mime", "default", desktop_id, "text/html"])
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
    let _ = Command::new("sudo")
        .args(["-u", user, "mkdir", "-p", &config_dir])
        .status();

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
        format!("text/html={desktop_id}"),
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
