use anyhow::Result;
use std::env;
use std::error::Error;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;

const LOG_DIR_ENV: &str = "MASH_BRAVE_DEFAULT_LOG_DIR";
const REPO_FILE_ENV: &str = "MASH_BRAVE_DEFAULT_REPO_FILE";

pub fn run(args: &[String]) -> Result<()> {
    let user = args.first().map(String::as_str).unwrap_or("drtweak");
    run_brave_default(user).map_err(|err| anyhow::anyhow!(err.to_string()))?;
    Ok(())
}

fn run_brave_default(user: &str) -> Result<(), Box<dyn Error>> {
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

    log(&mut log_file, "=== Brave install + default browser ===")?;

    let mut curl_check = Command::new("curl");
    curl_check.args(["-fsSL", "--max-time", "8", "https://www.google.com"]);
    if !command_success(&mut curl_check)? {
        log(
            &mut log_file,
            "No internet detected; skipping Brave install for now.",
        )?;
        return Ok(());
    }

    log(&mut log_file, "Installing Brave repo...")?;
    let mut rpm_import = Command::new("sudo");
    rpm_import.args([
        "rpm",
        "--import",
        "https://brave-browser-rpm-release.s3.brave.com/brave-core.asc",
    ]);
    run_command_ignore_failure(&mut rpm_import)?;

    let mut curl_repo = Command::new("curl");
    curl_repo.args([
        "-fsSL",
        "https://brave-browser-rpm-release.s3.brave.com/brave-browser.repo",
    ]);
    let output = curl_repo.output()?;
    if output.status.success() {
        if let Some(parent) = repo_file.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(&repo_file, output.stdout)?;
    }

    log(&mut log_file, "dnf install brave-browser")?;
    let mut dnf_install = Command::new("sudo");
    dnf_install.args(["dnf", "install", "-y", "brave-browser"]);
    if !command_success(&mut dnf_install)? {
        log(&mut log_file, "Brave install failed (repo/arch?).")?;
        return Ok(());
    }

    log(
        &mut log_file,
        &format!("Setting default browser for user: {user}"),
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

        if let Ok(home_dir) = get_home_dir(user) {
            let config_dir = home_dir.join(".config");
            fs::create_dir_all(&config_dir)?;
            let mimefile = config_dir.join("mimeapps.list");
            ensure_default_applications_section(&mimefile)?;
            append_default_application(
                &mimefile,
                "x-scheme-handler/http",
                "brave-browser.desktop",
            )?;
            append_default_application(
                &mimefile,
                "x-scheme-handler/https",
                "brave-browser.desktop",
            )?;
            let mut chown = Command::new("chown");
            let owner = format!("{user}:{user}");
            chown.args([owner.as_str(), mimefile.to_string_lossy().as_ref()]);
            run_command_ignore_failure(&mut chown)?;
        }
    } else {
        log(
            &mut log_file,
            &format!("User {user} not found yet; default browser will be set later via Dojo."),
        )?;
    }

    log(&mut log_file, "Done.")?;
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

fn log(file: &mut std::fs::File, message: &str) -> Result<(), Box<dyn Error>> {
    let mut date_cmd = Command::new("date");
    date_cmd.arg("-Is");
    let timestamp = command_output(date_cmd).unwrap_or_else(|| "unknown".to_string());
    writeln!(file, "[{timestamp}] {message}")?;
    println!("[{timestamp}] {message}");
    Ok(())
}

fn command_output(mut cmd: Command) -> Option<String> {
    let output = cmd.output().ok()?;
    if !output.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if text.is_empty() {
        None
    } else {
        Some(text)
    }
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
