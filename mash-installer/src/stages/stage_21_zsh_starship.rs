use anyhow::Result;
use std::env;
use std::error::Error;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;

const STARSHIP_INIT_LINE: &str = "eval \"$(starship init zsh)\"";

pub fn run(args: &[String]) -> Result<()> {
    let user = args.first().map(String::as_str).unwrap_or("DrTweak");
    println!("[*] Zsh + Starship (fallback installer)");
    setup_zsh_starship(user).map_err(|err| anyhow::anyhow!(err.to_string()))?;
    Ok(())
}

pub fn setup_zsh_starship(user: &str) -> Result<(), Box<dyn Error>> {
    let mut dnf = Command::new("dnf");
    dnf.args(["install", "-y", "--skip-unavailable", "zsh"]);
    run_command_ignore_failure(&mut dnf)?;

    if !path_has_executable("starship") {
        let mut installer = Command::new("sh");
        installer
            .arg("-c")
            .arg("curl -fsSL https://starship.rs/install.sh | sh -s -- -y");
        run_command_ignore_failure(&mut installer)?;
    }

    let home_dir = get_home_dir(user)?;
    fs::create_dir_all(&home_dir)?;

    let zshrc = home_dir.join(".zshrc");
    ensure_file_exists(&zshrc)?;
    ensure_starship_init_line(&zshrc)?;

    let mut chsh = Command::new("chsh");
    chsh.args(["-s", "/usr/bin/zsh", user]);
    run_command_ignore_failure(&mut chsh)?;

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

fn ensure_file_exists(path: &Path) -> Result<(), Box<dyn Error>> {
    if !path.exists() {
        OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(false)
            .open(path)?;
        if let Ok(metadata) = fs::metadata(path) {
            let mut perms = metadata.permissions();
            perms.set_mode(0o644);
            let _ = fs::set_permissions(path, perms);
        }
    }
    Ok(())
}

fn ensure_starship_init_line(path: &Path) -> Result<(), Box<dyn Error>> {
    let contents = fs::read_to_string(path).unwrap_or_default();
    if !contents.contains("starship init zsh") {
        let mut file = OpenOptions::new().append(true).create(true).open(path)?;
        writeln!(file, "{STARSHIP_INIT_LINE}")?;
    }
    Ok(())
}

fn path_has_executable(name: &str) -> bool {
    let paths = match env::var_os("PATH") {
        Some(paths) => paths,
        None => return false,
    };

    for dir in env::split_paths(&paths) {
        let candidate = dir.join(name);
        if let Ok(metadata) = fs::metadata(&candidate) {
            if metadata.is_file() && (metadata.permissions().mode() & 0o111) != 0 {
                return true;
            }
        }
    }

    false
}

fn run_command_ignore_failure(cmd: &mut Command) -> Result<(), Box<dyn Error>> {
    let status = cmd.status()?;
    if !status.success() {
        log::warn!("command exited with status {status}");
    }
    Ok(())
}
