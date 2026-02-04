use anyhow::{anyhow, Context, Result};
use mash_hal::ProcessOps;
use std::env;
use std::fs;
use std::path::PathBuf;
use std::time::Duration;

const NERD_FONT_VERSION: &str = "v3.3.0";

pub fn run(_args: &[String]) -> Result<()> {
    let hal = mash_hal::LinuxHal::new();

    println!("========================================");
    println!("  MASH Font Installation");
    println!("========================================\n");

    let ping_out = hal
        .command_output(
            "ping",
            &["-c1", "-W2", "github.com"],
            Duration::from_secs(5),
        )
        .context("failed to run ping")?;
    if !ping_out.status.success() {
        return Err(anyhow!("No internet connection detected."));
    }

    println!("Installing essential fonts for terminal and desktop...\n");

    println!("[1/4] Installing Terminus fonts (clean monospace)...");
    let _ = hal.command_status(
        "dnf",
        &["install", "-y", "terminus-fonts", "terminus-fonts-console"],
        Duration::from_secs(60 * 60),
    );

    println!("\n[2/4] Installing Noto Emoji fonts...");
    let _ = hal.command_status(
        "dnf",
        &[
            "install",
            "-y",
            "google-noto-emoji-fonts",
            "google-noto-emoji-color-fonts",
        ],
        Duration::from_secs(60 * 60),
    );

    println!("\n[3/4] Installing additional monospace fonts...");
    let _ = hal.command_status(
        "dnf",
        &[
            "install",
            "-y",
            "dejavu-sans-mono-fonts",
            "liberation-mono-fonts",
            "fira-code-fonts",
        ],
        Duration::from_secs(60 * 60),
    );

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
    let zip_path_s = zip_path.to_string_lossy().to_string();
    let font_dir_s = font_dir.to_string_lossy().to_string();
    let out = hal.command_output(
        "wget",
        &["-q", "--show-progress", &nerd_font_url, "-O", &zip_path_s],
        Duration::from_secs(10 * 60),
    );

    if let Ok(out) = out {
        if out.status.success() {
            let _ = hal.command_status(
                "unzip",
                &["-o", &zip_path_s, "-d", &font_dir_s],
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
    let _ = hal.command_status("fc-cache", &["-fv"], Duration::from_secs(5 * 60));

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
