use anyhow::Context;
use clap::Parser;

pub fn run() -> anyhow::Result<()> {
    let cli = mash_core::cli::Cli::parse();
    mash_core::logging::init_with(cli.log_file.clone());

    if cli.dump_tui {
        mash_tui::dojo::dump_all_steps()?;
        return Ok(());
    }

    if let Some(stage) = cli.stage.as_deref() {
        log::info!("🧩 Running stage: {}", stage);
        mash_core::stages::run_stage(stage, &cli.stage_arg)?;
        return Ok(());
    }

    match &cli.command {
        // No subcommand = launch Dojo UI (default)
        None => {
            log::info!("🎉 Launching MASH Dojo UI...");
            mash_tui::dojo::run(&cli, cli.watch, cli.dry_run)?;
        }
        // Preflight checks
        Some(mash_core::cli::Command::Preflight) => {
            log::info!("🔍 Running preflight checks...");
            mash_workflow::preflight::run(&mash_workflow::preflight::PreflightConfig::default())?;
            return Ok(()); // Exit after preflight
        }
        // CLI flash mode (for scripting)
        Some(mash_core::cli::Command::Flash {
            image,
            disk,
            scheme,
            uefi_dir,
            auto_unmount,
            yes_i_know,
            locale,
            early_ssh,
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
                let mut progress = |_p: mash_core::downloader::DownloadProgress| true;
                let mut stage = |_msg: &str| {};
                mash_core::download_manager::fetch_uefi_bundle(
                    &downloads_dir,
                    &mut progress,
                    &mut stage,
                    None,
                )?;
                final_uefi_dir = Some(uefi_dest_dir);
            }

            if *download_image {
                log::info!("⬇️ Downloading Fedora image...");
                let mut progress = |_p: mash_core::downloader::DownloadProgress| true;
                let mut stage = |_msg: &str| {};
                let path = mash_core::download_manager::fetch_fedora_image(
                    &downloads_dir,
                    image_version,
                    image_edition,
                    &mut progress,
                    &mut stage,
                    None,
                )?;
                final_image_path = Some(path);
            }

            let parsed_locale = if let Some(l_str) = locale.as_ref() {
                Some(mash_core::locale::LocaleConfig::parse_from_str(l_str)?)
            } else {
                None
            };

            let cli_flash_config = mash_core::flash::FlashConfig {
                os_distro: Some("Fedora".to_string()),
                os_flavour: None,
                disk_identity: None,
                efi_source: Some(if *download_uefi {
                    "download".to_string()
                } else {
                    "local".to_string()
                }),
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
                locale: parsed_locale,
                early_ssh: *early_ssh,
                progress_tx: None,
                efi_size: efi_size.clone(),
                boot_size: boot_size.clone(),
                root_end: root_end.clone(),
                disk_stable_id: None,
                partition_approval_mode: mash_core::flash::PartitionApprovalMode::Global,
            };

            mash_core::flash::run_with_progress(&cli_flash_config, *yes_i_know)?;
            return Ok(()); // Exit after CLI flash
        }
        Some(mash_core::cli::Command::StageStarshipToml {
            stage_dir,
            starship_toml,
        }) => {
            log::info!(
                "🧭 Staging starship.toml -> {}",
                stage_dir.join("assets").display()
            );
            mash_core::stages::stage_03_stage_starship_toml::copy_starship_toml(
                stage_dir,
                starship_toml,
            )?;
            return Ok(());
        }
        Some(mash_core::cli::Command::Install {
            state,
            dry_run,
            execute,
            confirm: _confirm,
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
            let cfg = mash_workflow::installer::pipeline::InstallConfig {
                dry_run: *dry_run,
                execute: *execute,
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

            let plan = mash_workflow::installer::pipeline::run_pipeline(&cfg)?;
            log::info!("{}", plan);
            return Ok(());
        }
    }

    Ok(())
}

fn parse_mount_spec(spec: &str) -> Option<mash_workflow::installer::pipeline::MountSpec> {
    let mut parts = spec.split(':');
    let device = parts.next()?.to_string();
    let target = parts.next()?.to_string();
    let fstype = parts.next().map(|s| s.to_string());
    Some(mash_workflow::installer::pipeline::MountSpec {
        device,
        target,
        fstype,
    })
}
