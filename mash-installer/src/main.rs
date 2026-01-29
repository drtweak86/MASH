//! 🍠 MASH - Fedora KDE for Raspberry Pi 4B
//!
//! A friendly TUI wizard for installing Fedora KDE on Raspberry Pi 4 with UEFI boot.
//! Run without arguments to launch the interactive TUI wizard.

#![allow(dead_code)] // Future use
#![allow(clippy::too_many_arguments)] // Installer config has many params

use anyhow::Context;
use clap::Parser;

use crossterm::{
    event::{DisableMouseCapture, EnableMouseCapture},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};
use std::{io, sync::mpsc, thread};

mod cli;
mod download;
mod errors;
mod flash;
mod locale;
mod logging;
mod preflight;
mod tui;

fn main() -> anyhow::Result<()> {
    logging::init();
    let cli = cli::Cli::parse();

    match &cli.command {
        // No subcommand = launch TUI wizard (default)
        None => {
            log::info!("🎉 Launching MASH TUI wizard...");
            let maybe_flash_config = tui::run(&cli, cli.watch, cli.dry_run)?;

            if let Some(mut flash_config) = maybe_flash_config {
                log::info!("TUI wizard completed. Starting installation...");

                // --- Handle Downloads ---
                let downloads_dir = cli.mash_root.join("downloads");

                // Download UEFI firmware if selected
                if flash_config.download_uefi_firmware {
                    log::info!("⬇️ Downloading UEFI firmware...");
                    let uefi_dest_dir = downloads_dir.join("uefi");
                    download::download_uefi_firmware(&uefi_dest_dir)?;
                    flash_config.uefi_dir = uefi_dest_dir;
                }

                // Download Fedora image if selected
                use crate::tui::ImageSource; // Import ImageSource for matching
                if flash_config.image_source_selection == ImageSource::DownloadFedora {
                    log::info!(
                        "⬇️ Downloading Fedora {} {} image...",
                        flash_config.image_version,
                        flash_config.image_edition
                    );
                    let image_dest_dir = downloads_dir.join("images");
                    let downloaded_image_path = download::download_fedora_image(
                        &image_dest_dir,
                        &flash_config.image_version,
                        &flash_config.image_edition,
                    )?;
                    flash_config.image = downloaded_image_path;
                }
                // --- End Handle Downloads ---

                // --- Re-initialize TUI for Progress Display ---
                enable_raw_mode()?;
                let mut stdout = io::stdout();
                execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
                let backend = CrosstermBackend::new(stdout);
                let mut terminal = Terminal::new(backend)?;

                // Create a temporary App state just for progress display
                let mut app_for_progress = crate::tui::App::new(&cli, cli.watch, cli.dry_run);
                app_for_progress.current_screen = crate::tui::Screen::Progress;
                app_for_progress.progress = tui::progress::ProgressState::default();

                // Create progress channel
                let (tx, rx) = mpsc::channel();
                app_for_progress.progress_rx = Some(rx);

                // Spawn flash thread
                let flash_config_clone = flash_config.clone(); // Clone for the thread
                let flash_handle = thread::spawn(move || {
                    flash::run_with_progress(
                        &flash_config_clone, // Pass the FlashConfig object
                        true,                // yes_i_know - already confirmed in TUI
                        Some(tx),            // progress_tx
                    )
                });

                // Run progress display loop
                let _ = crate::tui::run_progress_loop(&mut terminal, &mut app_for_progress);

                // Wait for flash to complete
                let flash_result = flash_handle
                    .join()
                    .map_err(|_| anyhow::anyhow!("Flash thread panicked"))?;

                // Restore terminal
                disable_raw_mode()?;
                execute!(
                    terminal.backend_mut(),
                    LeaveAlternateScreen,
                    DisableMouseCapture
                )?;
                terminal.show_cursor()?;

                // Report final flash result
                flash_result?;
            }
        }

        // Preflight checks
        Some(cli::Command::Preflight) => {
            log::info!("🔍 Running preflight checks...");
            preflight::run(cli.dry_run)?;
        }

        // CLI flash mode (for scripting)
        Some(cli::Command::Flash {
            image,
            disk,
            scheme,
            uefi_dir,
            auto_unmount,
            yes_i_know,
            locale: _locale,
            early_ssh: _early_ssh,
            efi_size,
            boot_size,
            root_end,
            download_uefi,
            download_image,
            image_version,
            image_edition,
        }) => {
            log::info!("💾 Running flash in CLI mode...");

            let mut final_image_path = image.clone();
            let mut final_uefi_dir = uefi_dir.clone();

            let downloads_dir = cli.mash_root.join("downloads");

            if *download_uefi {
                log::info!("⬇️ Downloading UEFI firmware...");
                let uefi_dest_dir = downloads_dir.join("uefi");
                download::download_uefi_firmware(&uefi_dest_dir)?;
                final_uefi_dir = Some(uefi_dest_dir);
            }

            if *download_image {
                log::info!("⬇️ Downloading Fedora image...");
                let image_dest_dir = downloads_dir.join("images");
                final_image_path = Some(download::download_fedora_image(
                    &image_dest_dir,
                    image_version,
                    image_edition,
                )?);
            }

            let parsed_locale = if let Some(l_str) = _locale.as_ref() {
                // Use .as_ref() to get &String
                Some(locale::LocaleConfig::parse_from_str(l_str)?)
            } else {
                None
            };

            flash::run(
                final_image_path
                    .as_ref()
                    .context("Image path is required (provide --image or use --download-image)")?,
                disk,
                *scheme,
                final_uefi_dir.as_ref().context(
                    "UEFI directory is required (provide --uefi-dir or use --download-uefi)",
                )?,
                cli.dry_run,
                *auto_unmount,
                *yes_i_know,
                parsed_locale, // Pass the parsed LocaleConfig
                *_early_ssh,
                efi_size,
                boot_size,
                root_end,
            )?;
        }
    }

    Ok(())
}
