use anyhow::{anyhow, Context, Result};
use mash_hal::ProcessOps;
use std::env;
use std::fs;
use std::path::PathBuf;
use std::time::Duration;

const NERD_FONT_VERSION: &str = "v3.3.0";

pub fn run(_args: &[String]) -> Result<()> {
    let hal = mash_hal::LinuxHal::new();

    log::info!("========================================");
    log::info!("  MASH Font Installation");
    log::info!("========================================\n");

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

    log::info!("Installing essential fonts for terminal and desktop...\n");

    log::info!("[1/4] Installing Terminus fonts (clean monospace)...");
    let _ = hal.command_status(
        "dnf",
        &["install", "-y", "terminus-fonts", "terminus-fonts-console"],
        Duration::from_secs(60 * 60),
    );

    log::info!("\n[2/4] Installing Noto Emoji fonts...");
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

    log::info!("\n[3/4] Installing additional monospace fonts...");
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

    log::info!("\n[4/4] Installing JetBrainsMono Nerd Font (for Starship prompt)...");
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
            log::info!(
                "  JetBrainsMono Nerd Font installed to {}",
                font_dir.display()
            );
        } else {
            log::warn!("Could not download Nerd Font. Skipping.");
        }
    } else {
        log::warn!("Could not download Nerd Font. Skipping.");
    }

    log::info!("\nRefreshing font cache...");
    let _ = hal.command_status("fc-cache", &["-fv"], Duration::from_secs(5 * 60));

    log::info!("\n========================================");
    log::info!("  Font Installation Complete!");
    log::info!("========================================\n");
    log::info!("Installed fonts:");
    log::info!("  - Terminus (terminal monospace)");
    log::info!("  - Noto Emoji (emoji support)");
    log::info!("  - DejaVu Sans Mono");
    log::info!("  - Liberation Mono");
    log::info!("  - Fira Code");
    log::info!("  - JetBrainsMono Nerd Font (Starship icons)");
    log::info!("\nTo use in terminal:");
    log::info!("  1. Open Konsole Preferences");
    log::info!("  2. Edit your Profile -> Appearance");
    log::info!("  3. Select 'JetBrainsMono Nerd Font' or 'Terminus'");
    log::info!("\nNerd Font is required for Starship prompt icons!");

    Ok(())
}
