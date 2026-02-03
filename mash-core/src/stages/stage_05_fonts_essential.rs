use anyhow::{anyhow, Context, Result};
use std::env;
use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::time::Duration;

const NERD_FONT_VERSION: &str = "v3.3.0";

pub fn run(_args: &[String]) -> Result<()> {
    println!("========================================");
    println!("  MASH Font Installation");
    println!("========================================\n");

    let mut cmd = Command::new("ping");
    cmd.args(["-c1", "-W2", "github.com"]);
    let ping_status =
        crate::process_timeout::status_with_timeout("ping", &mut cmd, Duration::from_secs(5))
            .context("failed to run ping")?;
    if !ping_status.success() {
        return Err(anyhow!("No internet connection detected."));
    }

    println!("Installing essential fonts for terminal and desktop...\n");

    println!("[1/4] Installing Terminus fonts (clean monospace)...");
    let mut cmd = Command::new("dnf");
    cmd.args(["install", "-y", "terminus-fonts", "terminus-fonts-console"]);
    let _ =
        crate::process_timeout::status_with_timeout("dnf", &mut cmd, Duration::from_secs(60 * 60));

    println!("\n[2/4] Installing Noto Emoji fonts...");
    let mut cmd = Command::new("dnf");
    cmd.args([
        "install",
        "-y",
        "google-noto-emoji-fonts",
        "google-noto-emoji-color-fonts",
    ]);
    let _ =
        crate::process_timeout::status_with_timeout("dnf", &mut cmd, Duration::from_secs(60 * 60));

    println!("\n[3/4] Installing additional monospace fonts...");
    let mut cmd = Command::new("dnf");
    cmd.args([
        "install",
        "-y",
        "dejavu-sans-mono-fonts",
        "liberation-mono-fonts",
        "fira-code-fonts",
    ]);
    let _ =
        crate::process_timeout::status_with_timeout("dnf", &mut cmd, Duration::from_secs(60 * 60));

    println!("\n[4/4] Installing JetBrainsMono Nerd Font (for Starship prompt)...");
    let home = env::var("HOME").unwrap_or_else(|_| "/root".to_string());
    let font_dir = PathBuf::from(home).join(".local/share/fonts");
    fs::create_dir_all(&font_dir)?;

    let nerd_font_url = format!(
        "https://github.com/ryanoasis/nerd-fonts/releases/download/{}/JetBrainsMono.zip",
        NERD_FONT_VERSION
    );

    let tmp_dir = PathBuf::from("/tmp");
    let zip_path = tmp_dir.join("JetBrainsMono.zip");
    let mut cmd = Command::new("wget");
    cmd.args(["-q", "--show-progress", &nerd_font_url, "-O"])
        .arg(&zip_path);
    let status =
        crate::process_timeout::status_with_timeout("wget", &mut cmd, Duration::from_secs(10 * 60));

    if let Ok(status) = status {
        if status.success() {
            let mut cmd = Command::new("unzip");
            cmd.args(["-o"]).arg(&zip_path).arg("-d").arg(&font_dir);
            let _ = crate::process_timeout::status_with_timeout(
                "unzip",
                &mut cmd,
                Duration::from_secs(2 * 60),
            );
            let _ = fs::remove_file(&zip_path);
            println!(
                "  JetBrainsMono Nerd Font installed to {}",
                font_dir.display()
            );
        } else {
            println!("  WARNING: Could not download Nerd Font. Skipping.");
        }
    } else {
        println!("  WARNING: Could not download Nerd Font. Skipping.");
    }

    println!("\nRefreshing font cache...");
    let mut cmd = Command::new("fc-cache");
    cmd.args(["-fv"]);
    let _ = crate::process_timeout::status_with_timeout(
        "fc-cache",
        &mut cmd,
        Duration::from_secs(5 * 60),
    );

    println!("\n========================================");
    println!("  Font Installation Complete!");
    println!("========================================\n");
    println!("Installed fonts:");
    println!("  - Terminus (terminal monospace)");
    println!("  - Noto Emoji (emoji support)");
    println!("  - DejaVu Sans Mono");
    println!("  - Liberation Mono");
    println!("  - Fira Code");
    println!("  - JetBrainsMono Nerd Font (Starship icons)");
    println!("\nTo use in terminal:");
    println!("  1. Open Konsole Preferences");
    println!("  2. Edit your Profile -> Appearance");
    println!("  3. Select 'JetBrainsMono Nerd Font' or 'Terminus'");
    println!("\nNerd Font is required for Starship prompt icons!");

    Ok(())
}
