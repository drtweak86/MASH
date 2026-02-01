use anyhow::Result;
use std::fs;
use std::io::Write;
use std::process::Command;

pub fn run(args: &[String]) -> Result<()> {
    let user = args.first().map(String::as_str).unwrap_or("DrTweak");
    println!("[*] Zsh + Starship (fallback installer)");

    let _ = Command::new("dnf")
        .args(["install", "-y", "--skip-unavailable", "zsh"])
        .status();

    let starship_installed = Command::new("sh")
        .args(["-lc", "command -v starship"])
        .status()
        .map(|s| s.success())
        .unwrap_or(false);

    if !starship_installed {
        let _ = Command::new("sh")
            .args([
                "-lc",
                "curl -fsSL https://starship.rs/install.sh | sh -s -- -y",
            ])
            .status();
    }

    let output = Command::new("getent").args(["passwd", user]).output()?;
    let line = String::from_utf8_lossy(&output.stdout);
    let home = line.split(':').nth(5).unwrap_or("").trim();
    if home.is_empty() {
        return Ok(());
    }

    fs::create_dir_all(home)?;
    let zshrc = format!("{home}/.zshrc");
    let mut file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&zshrc)?;
    let current = fs::read_to_string(&zshrc).unwrap_or_default();
    if !current.contains("starship init zsh") {
        writeln!(file, "eval \"$(starship init zsh)\"")?;
    }

    let _ = Command::new("chsh")
        .args(["-s", "/usr/bin/zsh", user])
        .status();

    Ok(())
}
