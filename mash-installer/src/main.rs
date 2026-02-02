//! 🍠 MASH - Fedora KDE for Raspberry Pi 4B
//!
//! A friendly TUI wizard for installing Fedora KDE on Raspberry Pi 4 with UEFI boot.
//! Run without arguments to launch the interactive TUI wizard.

#![allow(dead_code)] // Future use
#![allow(clippy::too_many_arguments)] // Installer config has many params

use anyhow::Context;
use clap::Parser;

mod cli;
mod disk_ops;
mod download;
mod errors;
mod flash;
mod locale;
mod logging;
mod preflight;
mod stages;
mod tui;
mod ui;

fn main() -> anyhow::Result<()> {
    logging::init();
    let cli = cli::Cli::parse();

    if cli.dump_tui {
        tui::dump_all_steps()?;
        return Ok(());
    }

    if let Some(stage) = cli.stage.as_deref() {
        log::info!("{} Running stage: {}", ui::style::emoji::ACTION, stage);
        stages::run_stage(stage, &cli.stage_arg)?;
        return Ok(());
    }

    if cli.dry_run && cli.command.is_none() {
        log::info!("DRY RUN: Executing disk operations sequence.");
        let disks = disk_ops::probe_disks(true)?;
        if let Some(disk) = disks.first() {
            let plan = disk_ops::plan_partitioning(disk, true)?;
            disk_ops::format_partitions(&plan, true)?;
            disk_ops::mount_partitions(&plan, true)?;
            disk_ops::verify_disk_operations(&plan, true)?;
        } else {
            log::info!("DRY RUN: No disks found to plan.");
        }
        return Ok(());
    }

    match &cli.command {
        // No subcommand = launch TUI wizard (default)
        None => {
            log::info!("{} Launching MASH TUI wizard...", ui::style::emoji::PARTY);
            tui::run(&cli, cli.watch, cli.dry_run)?;
        }
        // Preflight checks
        Some(cli::Command::Preflight) => {
            log::info!("{} Running preflight checks...", ui::style::emoji::SEARCH);
            preflight::run(cli.dry_run)?;
            return Ok(()); // Exit after preflight
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
            log::info!("{} Running flash in CLI mode...", ui::style::emoji::DISK);

            let mut final_image_path = image.clone();
            let mut final_uefi_dir = uefi_dir.clone();

            let downloads_dir = cli.mash_root.join("downloads");

            if *download_uefi {
                log::info!(
                    "{} Downloading UEFI firmware...",
                    ui::style::emoji::DOWNLOAD
                );
                let uefi_dest_dir = downloads_dir.join("uefi");
                download::download_uefi_firmware(&uefi_dest_dir)?;
                final_uefi_dir = Some(uefi_dest_dir);
            }

            if *download_image {
                log::info!("{} Downloading Fedora image...", ui::style::emoji::DOWNLOAD);
                let image_dest_dir = downloads_dir.join("images");
                final_image_path = Some(download::download_fedora_image(
                    &image_dest_dir,
                    image_version,
                    image_edition,
                )?);
            }

            let parsed_locale = if let Some(l_str) = _locale.as_ref() {
                Some(locale::LocaleConfig::parse_from_str(l_str)?)
            } else {
                None
            };

            let cli_flash_config = tui::FlashConfig {
                image: final_image_path
                    .as_ref()
                    .context("Image path is required (provide --image or use --download-image)")?
                    .clone(),
                disk: disk.clone(),
                scheme: *scheme,
                uefi_dir: final_uefi_dir
                    .as_ref()
                    .context(
                        "UEFI directory is required (provide --uefi-dir or use --download-uefi)",
                    )?
                    .clone(),
                dry_run: cli.dry_run,
                auto_unmount: *auto_unmount,
                watch: cli.watch,
                locale: parsed_locale,
                early_ssh: *_early_ssh,
                progress_tx: None,
                efi_size: efi_size.clone(),
                boot_size: boot_size.clone(),
                root_end: root_end.clone(),
                download_uefi_firmware: *download_uefi,
                image_source_selection: if *download_image {
                    tui::ImageSource::DownloadFedora
                } else {
                    tui::ImageSource::LocalFile
                },
                image_version: image_version.clone(),
                image_edition: image_edition.clone(),
            };

            let mut confirmed = *yes_i_know;
            if !confirmed && !cli.dry_run {
                let prompt = format!(
                    "WARNING: This will erase ALL data on {}. There is no undo. Continue?",
                    disk
                );
                let result = ui::confirm::confirm_and_run(&prompt, || Ok(()))?;
                if !result {
                    log::info!("{} Installation aborted by user.", ui::style::emoji::CANCEL);
                    return Ok(());
                }
                confirmed = true;
            }

            flash::run_with_progress(&cli_flash_config, confirmed)?;
            return Ok(()); // Exit after CLI flash
        }
    }

    Ok(())
}
