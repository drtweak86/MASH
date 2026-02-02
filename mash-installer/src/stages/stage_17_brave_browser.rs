use anyhow::Result;
use std::env;
use std::error::Error;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;

const LOG_DIR_ENV: &str = "MASH_BRAVE_LOG_DIR";
const REPO_FILE_ENV: &str = "MASH_BRAVE_REPO_FILE";

const REPO_CONTENT: &str = "[brave-browser]\nname=Brave Browser\nbaseurl=https://brave-browser-rpm-release.s3.brave.com/x86_64/\nenabled=1\ngpgcheck=1\ngpgkey=https://brave-browser-rpm-release.s3.brave.com/brave-core.asc\n";

pub fn run(args: &[String]) -> Result<()> {
    let user = args.first().map(String::as_str).unwrap_or("drtweak");
    run_brave_browser(user).map_err(|err| anyhow::anyhow!(err.to_string()))?;
    Ok(())
}

fn run_brave_browser(user: &str) -> Result<(), Box<dyn Error>> {
    let log_dir = env::var(LOG_DIR_ENV).unwrap_or_else(|_| "/data/mash-logs".to_string());
    let repo_file = env::var(REPO_FILE_ENV)
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("/etc/yum.repos.d/brave-browser.repo"));

    fs::create_dir_all(&log_dir)?;
    let log_path = Path::new(&log_dir).join("brave.log");
    let mut log_file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_path)?;

    log_line(
        &mut log_file,
        "================================================================================",
    )?;
    log_line(&mut log_file, "🌐 Brave browser + default browser setup")?;
    log_line(
        &mut log_file,
        "================================================================================",
    )?;

    let mut dnf_core = Command::new("dnf");
    dnf_core.args([
        "install",
        "-y",
        "--setopt=install_weak_deps=True",
        "dnf-plugins-core",
        "curl",
        "ca-certificates",
    ]);
    run_command_ignore_failure(&mut dnf_core)?;

    if !repo_file.exists() {
        log_line(
            &mut log_file,
            &format!("➕ Adding Brave repo: {}", repo_file.display()),
        )?;
        let mut rpm_import = Command::new("rpm");
        rpm_import.args([
            "--import",
            "https://brave-browser-rpm-release.s3.brave.com/brave-core.asc",
        ]);
        run_command_ignore_failure(&mut rpm_import)?;
        if let Some(parent) = repo_file.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(&repo_file, REPO_CONTENT)?;
    }

    log_line(
        &mut log_file,
        "📦 Installing brave-browser (best-effort; may be unavailable on aarch64)",
    )?;
    let mut dnf_brave = Command::new("dnf");
    dnf_brave.args(["install", "-y", "--skip-unavailable", "brave-browser"]);
    run_command_ignore_failure(&mut dnf_brave)?;

    let mut rpm_brave = Command::new("rpm");
    rpm_brave.args(["-q", "brave-browser"]);
    if !command_success(&mut rpm_brave)? {
        log_line(
            &mut log_file,
            "⚠️ brave-browser not installed (likely no aarch64 build in repo).",
        )?;
        log_line(
            &mut log_file,
            "   Dojo will offer alternatives (Firefox) and you can revisit later.",
        )?;
        let mut dnf_firefox = Command::new("dnf");
        dnf_firefox.args([
            "install",
            "-y",
            "--setopt=install_weak_deps=True",
            "firefox",
        ]);
        run_command_ignore_failure(&mut dnf_firefox)?;

        let mut rpm_firefox = Command::new("rpm");
        rpm_firefox.args(["-q", "firefox"]);
        if command_success(&mut rpm_firefox)? {
            log_line(
                &mut log_file,
                "🦊 Falling back to Firefox as default browser.",
            )?;
            let mut id_cmd = Command::new("id");
            id_cmd.arg(user);
            if command_success(&mut id_cmd)? {
                let mut xdg_settings = Command::new("sudo");
                xdg_settings.args([
                    "-u",
                    user,
                    "xdg-settings",
                    "set",
                    "default-web-browser",
                    "firefox.desktop",
                ]);
                run_command_ignore_failure(&mut xdg_settings)?;

                let mut xdg_mime_http = Command::new("sudo");
                xdg_mime_http.args([
                    "-u",
                    user,
                    "xdg-mime",
                    "default",
                    "firefox.desktop",
                    "x-scheme-handler/http",
                ]);
                run_command_ignore_failure(&mut xdg_mime_http)?;

                let mut xdg_mime_https = Command::new("sudo");
                xdg_mime_https.args([
                    "-u",
                    user,
                    "xdg-mime",
                    "default",
                    "firefox.desktop",
                    "x-scheme-handler/https",
                ]);
                run_command_ignore_failure(&mut xdg_mime_https)?;

                let mut xdg_mime_html = Command::new("sudo");
                xdg_mime_html.args([
                    "-u",
                    user,
                    "xdg-mime",
                    "default",
                    "firefox.desktop",
                    "text/html",
                ]);
                run_command_ignore_failure(&mut xdg_mime_html)?;
            }
        }
        return Ok(());
    }

    log_line(
        &mut log_file,
        &format!("🔧 Setting default browser to Brave for user: {user}"),
    )?;

    let mut id_cmd = Command::new("id");
    id_cmd.arg(user);
    if command_success(&mut id_cmd)? {
        let mut xdg_settings = Command::new("sudo");
        xdg_settings.args([
            "-u",
            user,
            "xdg-settings",
            "set",
            "default-web-browser",
            "brave-browser.desktop",
        ]);
        run_command_ignore_failure(&mut xdg_settings)?;

        let mut xdg_mime_http = Command::new("sudo");
        xdg_mime_http.args([
            "-u",
            user,
            "xdg-mime",
            "default",
            "brave-browser.desktop",
            "x-scheme-handler/http",
        ]);
        run_command_ignore_failure(&mut xdg_mime_http)?;

        let mut xdg_mime_https = Command::new("sudo");
        xdg_mime_https.args([
            "-u",
            user,
            "xdg-mime",
            "default",
            "brave-browser.desktop",
            "x-scheme-handler/https",
        ]);
        run_command_ignore_failure(&mut xdg_mime_https)?;

        let mut xdg_mime_html = Command::new("sudo");
        xdg_mime_html.args([
            "-u",
            user,
            "xdg-mime",
            "default",
            "brave-browser.desktop",
            "text/html",
        ]);
        run_command_ignore_failure(&mut xdg_mime_html)?;

        if let Ok(home_dir) = get_home_dir(user) {
            let config_dir = home_dir.join(".config");
            fs::create_dir_all(&config_dir)?;
            let mimeapps = config_dir.join("mimeapps.list");
            ensure_default_applications_section(&mimeapps)?;
            append_default_application(
                &mimeapps,
                "x-scheme-handler/http",
                "brave-browser.desktop",
            )?;
            append_default_application(
                &mimeapps,
                "x-scheme-handler/https",
                "brave-browser.desktop",
            )?;
            append_default_application(&mimeapps, "text/html", "brave-browser.desktop")?;
            let mut chown = Command::new("chown");
            let owner = format!("{user}:{user}");
            chown.args([owner.as_str(), mimeapps.to_string_lossy().as_ref()]);
            run_command_ignore_failure(&mut chown)?;
        }
    } else {
        log_line(
            &mut log_file,
            &format!("⚠️ user '{user}' not found yet; skipping default-browser binding."),
        )?;
    }

    log_line(&mut log_file, "✅ Brave step complete.")?;

    Ok(())
}

fn ensure_default_applications_section(path: &Path) -> Result<(), Box<dyn Error>> {
    if path.exists() {
        let contents = fs::read_to_string(path)?;
        if contents.contains("[Default Applications]") {
            return Ok(());
        }
    }
    let mut file = OpenOptions::new().create(true).append(true).open(path)?;
    writeln!(file, "[Default Applications]")?;
    Ok(())
}

fn append_default_application(path: &Path, key: &str, value: &str) -> Result<(), Box<dyn Error>> {
    let contents = fs::read_to_string(path).unwrap_or_default();
    if contents
        .lines()
        .any(|line| line.starts_with(&format!("{key}=")))
    {
        return Ok(());
    }
    let mut file = OpenOptions::new().append(true).create(true).open(path)?;
    writeln!(file, "{key}={value}")?;
    Ok(())
}

fn get_home_dir(user: &str) -> Result<PathBuf, Box<dyn Error>> {
    let output = Command::new("getent").args(["passwd", user]).output()?;
    if !output.status.success() {
        return Err(format!("getent passwd failed for user {user}").into());
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    let entry = stdout.lines().next().ok_or("empty getent output")?;
    let parts: Vec<&str> = entry.split(':').collect();
    if parts.len() < 6 {
        return Err(format!("unexpected getent passwd format for {user}").into());
    }
    Ok(PathBuf::from(parts[5]))
}

fn log_line(file: &mut std::fs::File, message: &str) -> Result<(), Box<dyn Error>> {
    writeln!(file, "{message}")?;
    println!("{message}");
    Ok(())
}

fn run_command_ignore_failure(cmd: &mut Command) -> Result<(), Box<dyn Error>> {
    let status = cmd.status()?;
    if !status.success() {
        log::warn!("command exited with status {status}");
    }
    Ok(())
}

fn command_success(cmd: &mut Command) -> Result<bool, Box<dyn Error>> {
    let status = cmd.status()?;
    Ok(status.success())
}
