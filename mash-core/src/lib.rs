//! 🍠 MASH - Fedora KDE for Raspberry Pi 4B
//!
//! A friendly TUI wizard for installing Fedora KDE on Raspberry Pi 4 with UEFI boot.
//! Run without arguments to launch the interactive TUI wizard.

#![allow(dead_code)] // Future use
#![allow(clippy::too_many_arguments)] // Installer config has many params

use anyhow::Context;
use clap::Parser;

pub mod boot_config;
mod cli;
pub mod disk_ops;
mod download;
pub mod downloader;
mod errors;
mod flash;
pub mod installer;
mod locale;
mod logging;
mod preflight;
pub mod stage_runner;
mod stages;
pub mod state_manager;
pub mod system_config;
mod tui;

pub fn run() -> anyhow::Result<()> {
    logging::init();
    let cli = cli::Cli::parse();

    if cli.dump_tui {
        tui::dump_all_steps()?;
        return Ok(());
    }

    if let Some(stage) = cli.stage.as_deref() {
        log::info!("🧩 Running stage: {}", stage);
        stages::run_stage(stage, &cli.stage_arg)?;
        return Ok(());
    }

    match &cli.command {
        // No subcommand = launch TUI wizard (default)
        None => {
            log::info!("🎉 Launching MASH TUI wizard...");
            tui::run(&cli, cli.watch, cli.dry_run)?;
        }
        // Preflight checks
        Some(cli::Command::Preflight) => {
            log::info!("🔍 Running preflight checks...");
            preflight::run(&preflight::PreflightConfig::default())?;
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

            flash::run_with_progress(&cli_flash_config, *yes_i_know)?;
            return Ok(()); // Exit after CLI flash
        }
        Some(cli::Command::StageStarshipToml {
            stage_dir,
            starship_toml,
        }) => {
            log::info!(
                "🧭 Staging starship.toml -> {}",
                stage_dir.join("assets").display()
            );
            stages::stage_03_stage_starship_toml::copy_starship_toml(stage_dir, starship_toml)?;
            return Ok(());
        }
        Some(cli::Command::Install {
            state,
            dry_run,
            execute,
            confirm,
            disk,
            mount,
            format_ext4,
            format_btrfs,
            package,
            kernel_fix,
            kernel_fix_root,
            mountinfo_path,
            by_uuid_path,
            reboots,
            download_mirror,
            download_checksum,
            download_checksum_url,
            download_timeout_secs,
            download_retries,
            download_dir,
        }) => {
            let mounts = mount
                .iter()
                .filter_map(|spec| parse_mount_spec(spec))
                .collect::<Vec<_>>();
            let download_dir = cli.mash_root.join(download_dir);
            let cfg = installer::pipeline::InstallConfig {
                dry_run: *dry_run,
                execute: *execute,
                confirmed: *confirm,
                state_path: state.clone(),
                disk: disk.clone(),
                mounts,
                format_ext4: format_ext4.clone(),
                format_btrfs: format_btrfs.clone(),
                packages: package.clone(),
                kernel_fix: *kernel_fix,
                kernel_fix_root: kernel_fix_root.clone(),
                mountinfo_path: mountinfo_path.clone(),
                by_uuid_path: by_uuid_path.clone(),
                reboot_count: *reboots,
                mash_root: cli.mash_root.clone(),
                download_image: false,
                download_uefi: false,
                image_version: "43".to_string(),
                image_edition: "KDE".to_string(),
                download_mirror: download_mirror.clone(),
                download_checksum: download_checksum.clone(),
                download_checksum_url: download_checksum_url.clone(),
                download_timeout_secs: *download_timeout_secs,
                download_retries: *download_retries,
                download_dir,
            };

            let plan = installer::pipeline::run_pipeline(&cfg)?;
            println!("{}", plan);
            return Ok(());
        }
    }

    Ok(())
}

fn parse_mount_spec(spec: &str) -> Option<installer::pipeline::MountSpec> {
    let mut parts = spec.split(':');
    let device = parts.next()?.to_string();
    let target = parts.next()?.to_string();
    let fstype = parts.next().map(|s| s.to_string());
    Some(installer::pipeline::MountSpec {
        device,
        target,
        fstype,
    })
}
