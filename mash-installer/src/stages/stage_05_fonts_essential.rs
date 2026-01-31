use anyhow::{anyhow, Context, Result};
use std::env;
use std::fs;
use std::path::PathBuf;
use std::process::Command;

const NERD_FONT_VERSION: &str = "v3.3.0";

pub fn run(_args: &[String]) -> Result<()> {
    println!("========================================");
    println!("  MASH Font Installation");
    println!("========================================\n");

    let ping_status = Command::new("ping")
        .args(["-c1", "-W2", "github.com"])
        .status()
        .context("failed to run ping")?;
    if !ping_status.success() {
        return Err(anyhow!("No internet connection detected."));
    }

    println!("Installing essential fonts for terminal and desktop...\n");

    println!("[1/4] Installing Terminus fonts (clean monospace)...");
    let _ = Command::new("dnf")
        .args(["install", "-y", "terminus-fonts", "terminus-fonts-console"])
        .status();

    println!("\n[2/4] Installing Noto Emoji fonts...");
    let _ = Command::new("dnf")
        .args([
            "install",
            "-y",
            "google-noto-emoji-fonts",
            "google-noto-emoji-color-fonts",
        ])
        .status();

    println!("\n[3/4] Installing additional monospace fonts...");
    let _ = Command::new("dnf")
        .args([
            "install",
            "-y",
            "dejavu-sans-mono-fonts",
            "liberation-mono-fonts",
            "fira-code-fonts",
        ])
        .status();

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
    let status = Command::new("wget")
        .args(["-q", "--show-progress", &nerd_font_url, "-O"])
        .arg(&zip_path)
        .status();

    if let Ok(status) = status {
        if status.success() {
            let _ = Command::new("unzip")
                .args(["-o"])
                .arg(&zip_path)
                .arg("-d")
                .arg(&font_dir)
                .status();
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
    let _ = Command::new("fc-cache").args(["-fv"]).status();

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
