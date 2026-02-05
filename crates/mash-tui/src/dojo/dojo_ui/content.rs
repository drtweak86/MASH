use super::super::dojo_app::{App, CustomizeField, DiskOption, InstallStepType};
use super::super::{data_sources, flash_config};
use crate::progress::{Phase, ProgressState};
use mash_core::flash::PartitionApprovalMode;
use std::path::PathBuf;

pub(super) fn build_info_panel(app: &App, progress_state: &ProgressState) -> String {
    let mut lines = Vec::new();

    // Mode status (prominent; DRY-RUN must not look destructive)
    if app.dry_run {
        lines.push("🟡 DRY-RUN (no disk writes)".to_string());
    } else {
        lines.push("🔴 EXECUTE (disk writes possible)".to_string());
    }
    lines.push("".to_string());

    // Safety status (prominent)
    let arming_status = if app.destructive_armed {
        "🔴 ARMED (writes enabled)"
    } else {
        "🟢 SAFE (disarmed)"
    };
    lines.push(arming_status.to_string());
    lines.push("".to_string());

    // Current selections
    lines.push("Selections:".to_string());

    // OS selection
    if let Some(distro) = app.os_distros.get(app.os_distro_selected) {
        lines.push(format!("OS: {}", distro.display()));
    }

    // Variant selection
    if let Some(variant) = app.os_variants.get(app.os_variant_selected) {
        lines.push(format!("Variant: {}", variant.label));
    }

    // Disk selection
    if let Some(disk) = app.disks.get(app.disk_selected) {
        lines.push(format!("Disk: {} ({})", disk.label, disk.path));
    }

    // Partition scheme
    if let Some(scheme) = app.partition_schemes.get(app.scheme_selected) {
        let scheme_str = match scheme {
            mash_core::cli::PartitionScheme::Mbr => "MBR",
            mash_core::cli::PartitionScheme::Gpt => "GPT",
        };
        lines.push(format!("Scheme: {}", scheme_str));
    }

    // Progress info (if running)
    if app.is_running {
        lines.push("".to_string());
        lines.push("Progress:".to_string());
        let percent = progress_state.overall_percent.round().clamp(0.0, 100.0);
        lines.push(format!("Overall: {:.0}%", percent));
        let phase_percent = progress_state.phase_percent.round().clamp(0.0, 100.0);
        lines.push(format!("Phase: {:.0}%", phase_percent));
        lines.push(format!("ETA: {}", progress_state.eta_string()));
    }

    lines.join("\n")
}

pub(super) fn build_dojo_lines(app: &App) -> Vec<String> {
    let current_step_title = app.current_step_type.title();
    let mut items = Vec::new();
    items.push(format!("🧭 Step: {}", current_step_title));
    let progress_state = app.progress_state_snapshot();

    if app.partition_approval_mode != PartitionApprovalMode::Global {
        items.push("⚠️ Approvals not implemented (stub only)".to_string());
        items.push("".to_string());
    }
    match app.current_step_type {
        InstallStepType::Welcome => {
            items.push("👋 Welcome to MASH: a safe, guided installer.".to_string());
            items.push("".to_string());
            items.push(
                "🛡️ Safety: No disks will be modified unless the installer is ARMED.".to_string(),
            );
            items.push(format!(
                "Current state: {}",
                if app.destructive_armed {
                    "🔴 ARMED (writes enabled)"
                } else {
                    "🟢 SAFE (disarmed — read-only)"
                }
            ));
            items.push("".to_string());
            items.push("⌨️ Keys:".to_string());
            items.push("  ↑/↓ or j/k — Move selection up/down".to_string());
            items.push("  Space — Select/toggle current item".to_string());
            items.push("  Enter or Tab — Continue".to_string());
            items.push("  Esc — Quit".to_string());
            items.push("  ? — Help".to_string());
            push_options(
                &mut items,
                &app.welcome_options,
                app.welcome_index,
                Some(app.welcome_selected),
            );
        }
        InstallStepType::DiskSelection => {
            items.push("💽 Select a target disk:".to_string());
            items.push("".to_string());
            items.push("⌨️ Keys:".to_string());
            items.push("  ↑/↓ or j/k — Move selection up/down".to_string());
            items.push("  Space — Select current disk".to_string());
            items.push("  Enter or Tab — Continue".to_string());
            items.push("  Esc — Go back".to_string());
            items.push("  ? — Help".to_string());
            items.push("  r — Refresh disk list".to_string());
            items.push("".to_string());

            // Show warning banner if boot detection failed
            use data_sources::BootConfidence;
            if app
                .disks
                .iter()
                .any(|d| d.boot_confidence == BootConfidence::Unknown)
            {
                items.push("".to_string());
                items.push("⚠️ WARNING: Boot device detection failed!".to_string());
                items.push("Boot disk tags may be UNVERIFIED. Proceed with caution.".to_string());
                items.push("".to_string());
            }

            if app.disks.is_empty() {
                items.push("No disks detected. Press r to refresh.".to_string());
            }
            let options = app.disks.iter().map(format_disk_entry).collect::<Vec<_>>();
            push_options(
                &mut items,
                &options,
                app.disk_index,
                Some(app.disk_selected),
            );

            // Show protection banner for source/boot media.
            if let Some(disk) = app.disks.get(app.disk_index) {
                if disk.boot_confidence.is_boot() || disk.is_source_disk {
                    items.push("".to_string());
                    items.push("🛑 PROTECTED MEDIA selected.".to_string());
                    if app.developer_mode {
                        items.push(
                            "Developer mode is ON: you may proceed, but confirmation will require 'DESTROY BOOT DISK'."
                                .to_string(),
                        );
                    } else {
                        items.push(
                            "This disk cannot be selected. Re-run with --developer-mode to override (dangerous)."
                                .to_string(),
                        );
                    }
                }
            }
        }
        InstallStepType::DiskConfirmation => {
            let disk = app.disks.get(app.disk_selected);
            if let Some(disk) = disk {
                let is_boot_disk = disk.boot_confidence.is_boot() || disk.is_source_disk;
                let disk_info = disk.label.clone();

                if is_boot_disk {
                    items.push("⚠️⚠️⚠️ CRITICAL WARNING: BOOT DISK SELECTED ⚠️⚠️⚠️".to_string());
                    items.push("".to_string());
                    items.push(
                        "You are about to DESTROY the disk your system is running from!"
                            .to_string(),
                    );
                    items.push(
                        "This will make your system UNBOOTABLE and cause DATA LOSS!".to_string(),
                    );
                    items.push("".to_string());
                    items.push(format!(
                        "BOOT DEVICE TO BE WIPED: {} ({})",
                        disk.path, disk_info
                    ));
                    items.push("".to_string());
                    items.push(
                        "Type 'DESTROY BOOT DISK' to confirm, then press Enter/Tab • Esc to go back."
                            .to_string(),
                    );
                } else {
                    items.push("⚠️ Confirm disk destruction:".to_string());
                    items.push("".to_string());
                    items.push(format!("TARGET TO BE WIPED: {} ({})", disk.path, disk_info));
                    items.push("".to_string());
                    items.push("⌨️ Keys:".to_string());
                    items.push("  Type DESTROY (exactly)".to_string());
                    items.push("  Enter or Tab — Continue".to_string());
                    items.push("  Esc — Cancel and go back".to_string());
                    items.push("  ? — Help".to_string());
                    items.push("".to_string());
                }

                let required_text = if is_boot_disk {
                    "DESTROY BOOT DISK"
                } else {
                    "DESTROY"
                };
                let (display, progress, total) =
                    confirmation_progress(required_text, &app.wipe_confirmation);
                items.push(format!("Required: {}", required_text));
                items.push(format!("Typed   : {}", display));
                items.push(format!("Progress: {}/{}", progress, total));
            } else {
                items.push("No disk selected.".to_string());
            }
            if let Some(error) = &app.error_message {
                items.push(format!("❌ {}", error));
            }
        }
        InstallStepType::BackupConfirmation => {
            items.push("⚠️ This will erase data on the selected disk.".to_string());
            items.push("💾 Have you backed up your data?".to_string());
            if app.backup_confirmed {
                items.push("✅ Backup confirmed.".to_string());
            }
            items.push("".to_string());
            items.push("⌨️ Keys:".to_string());
            items.push("  ↑/↓ or j/k — Move selection up/down".to_string());
            items.push("  Space — Select current item".to_string());
            items.push("  Enter or Tab — Continue".to_string());
            items.push("  Esc — Go back".to_string());
            items.push("  ? — Help".to_string());
            push_options(
                &mut items,
                &[
                    "No, go back".to_string(),
                    "Yes, I have a backup".to_string(),
                ],
                app.backup_choice_index,
                Some(if app.backup_confirmed { 1 } else { 0 }),
            );
        }
        InstallStepType::PartitionScheme => {
            items.push("🧩 Select a partition scheme:".to_string());
            items.push("".to_string());
            items.push("MBR: Compatible with older systems, simpler structure".to_string());
            items.push("GPT: Modern standard, supports larger disks, UEFI-oriented".to_string());
            items.push("".to_string());
            items.push("⌨️ Keys:".to_string());
            items.push("  ↑/↓ or j/k — Switch between MBR and GPT".to_string());
            items.push("  Enter or Tab — Continue".to_string());
            items.push("  Esc — Go back".to_string());
            items.push("  ? — Help".to_string());
            let options = app
                .partition_schemes
                .iter()
                .map(format_partition_scheme)
                .collect::<Vec<_>>();
            push_options(
                &mut items,
                &options,
                app.scheme_index,
                Some(app.scheme_selected),
            );
        }
        InstallStepType::PartitionLayout => {
            items.push("📐 Partition Layout:".to_string());
            items.push("".to_string());
            items.push("Pick a default layout or customize manually:".to_string());
            items.push("".to_string());

            // Show layout options with clear names and descriptions
            let layout_options = app
                .partition_layouts
                .iter()
                .enumerate()
                .map(|(idx, _layout_str)| match idx {
                    0 => "Default A (recommended) — Large ROOT for desktop workstation".to_string(),
                    1 => "Default B (minimal) — Compact ROOT for embedded/testing".to_string(),
                    _ => format!("Layout {}", idx + 1),
                })
                .collect::<Vec<_>>();

            let mut options = layout_options;
            options.push("Manual layout".to_string());
            push_options(
                &mut items,
                &options,
                app.layout_index,
                Some(app.layout_selected),
            );

            // Show detailed preview of selected layout
            if app.layout_index < app.partition_layouts.len() {
                if let Some(layout) = app.partition_layouts.get(app.layout_index) {
                    items.push("".to_string());
                    items.push("Partition details:".to_string());
                    items.extend(format_layout_preview(layout));
                }
            } else {
                items.push("".to_string());
                items.push(
                    "Manual mode lets you edit partition sizes on the next screen.".to_string(),
                );
            }

            items.push("".to_string());
            items.push("⌨️ Keys:".to_string());
            items.push("  ↑/↓ or j/k — Move selection up/down".to_string());
            items.push("  Space — Select current item".to_string());
            items.push("  Enter or Tab — Continue".to_string());
            items.push("  Esc — Go back".to_string());
            items.push("  ? — Help".to_string());
        }
        InstallStepType::PartitionCustomize => {
            items.push("🛠️ Customize partitions:".to_string());
            items.push("".to_string());
            items.push("⌨️ Keys:".to_string());
            items.push("  ↑/↓ or j/k — Move selection up/down".to_string());
            items.push("  Type — Edit selected field".to_string());
            items.push("  Backspace — Delete".to_string());
            items.push("  R — Reset defaults".to_string());
            items.push("  Enter or Tab — Continue".to_string());
            items.push("  Esc — Go back".to_string());
            items.push("  ? — Help".to_string());
            let options = app
                .partition_customizations
                .iter()
                .enumerate()
                .map(|(idx, option)| {
                    let field = match idx {
                        0 => Some(CustomizeField::Efi),
                        1 => Some(CustomizeField::Boot),
                        2 => Some(CustomizeField::Root),
                        _ => None,
                    };
                    if field.is_some() && app.customize_error_field == field {
                        format!("❌ {}", option)
                    } else {
                        option.clone()
                    }
                })
                .collect::<Vec<_>>();
            push_options(&mut items, &options, app.customize_index, None);
            if let Some(error) = &app.error_message {
                items.push(format!("❌ {}", error));
            }
        }
        InstallStepType::DownloadSourceSelection => {
            items.push("📥 Select image source:".to_string());
            items.push("".to_string());
            items.push("⌨️ Keys:".to_string());
            items.push("  ↑/↓ or j/k — Move selection up/down".to_string());
            items.push("  Space — Select current item".to_string());
            items.push("  Enter or Tab — Continue".to_string());
            items.push("  Esc — Go back".to_string());
            items.push("  ? — Help".to_string());
            let options = app
                .image_sources
                .iter()
                .map(|source| source.label.clone())
                .collect::<Vec<_>>();
            push_options(
                &mut items,
                &options,
                app.image_source_index,
                Some(app.image_source_selected),
            );
            if app
                .image_sources
                .get(app.image_source_selected)
                .map(|source| source.value == flash_config::ImageSource::LocalFile)
                .unwrap_or(false)
            {
                items.push("Local image path:".to_string());
                items.push(app.image_source_path.clone());
            }
        }
        InstallStepType::ImageSelection => {
            items.push("🖼️ Select OS distribution:".to_string());
            items.push("".to_string());
            items.push("⌨️ Keys:".to_string());
            items.push("  ↑/↓ or j/k — Move selection up/down".to_string());
            items.push("  Space — Select current item".to_string());
            items.push("  Enter or Tab — Continue".to_string());
            items.push("  Esc — Go back".to_string());
            items.push("  ? — Help".to_string());
            items.push("".to_string());

            // Show OS distro options
            let options = app
                .os_distros
                .iter()
                .map(|distro| distro.display().to_string())
                .collect::<Vec<_>>();
            push_options(
                &mut items,
                &options,
                app.os_distro_index,
                Some(app.os_distro_selected),
            );
            items.push("".to_string());
            items.push("Next: pick a variant (server/desktop/etc).".to_string());
        }
        InstallStepType::VariantSelection => {
            items.push("🎛️ Select OS flavour/variant:".to_string());
            items.push("".to_string());
            items.push("Choose the edition or desktop environment for your OS.".to_string());
            items.push("".to_string());
            items.push("⌨️ Keys:".to_string());
            items.push("  ↑/↓ or j/k — Move selection up/down".to_string());
            items.push("  Space — Select current item".to_string());
            items.push("  Enter or Tab — Continue".to_string());
            items.push("  Esc — Go back to OS selection".to_string());
            items.push("  ? — Help".to_string());
            items.push("".to_string());

            if app.os_variants.is_empty() {
                items.push("".to_string());
                items.push("❌ Missing metadata: no variants found for this OS.".to_string());
                items.push("Press Esc to go back and choose a different OS.".to_string());
            } else {
                let options = app
                    .os_variants
                    .iter()
                    .map(|v| v.label.clone())
                    .collect::<Vec<_>>();
                push_options(
                    &mut items,
                    &options,
                    app.os_variant_index,
                    Some(app.os_variant_selected),
                );
            }
        }
        InstallStepType::EfiImage => {
            items.push("🧩 Choose how to get the EFI image:".to_string());
            items.push("".to_string());
            items.push("⌨️ Keys:".to_string());
            items.push("  ↑/↓ or j/k — Move selection up/down".to_string());
            items.push("  Space — Select current item".to_string());
            items.push("  Enter or Tab — Continue".to_string());
            items.push("  Esc — Go back".to_string());
            items.push("  ? — Help".to_string());

            let uefi_source = app.uefi_sources.get(app.uefi_source_selected);
            let is_local = matches!(uefi_source, Some(flash_config::EfiSource::LocalEfiImage));

            // Show EFI source options (intent-only)
            let options = app
                .uefi_sources
                .iter()
                .map(|source| source.display().to_string())
                .collect::<Vec<_>>();
            push_options(
                &mut items,
                &options,
                app.uefi_source_index,
                Some(app.uefi_source_selected),
            );

            // Only ask for a path if the user chose "Use local EFI image".
            if is_local {
                items.push("".to_string());
                items.push("Local EFI image path:".to_string());
                items.push(app.uefi_source_path.clone());
            }
        }
        InstallStepType::LocaleSelection => {
            items.push("🗣️ Select locale and keymap:".to_string());
            items.push("".to_string());
            items.push("⌨️ Keys:".to_string());
            items.push("  ↑/↓ or j/k — Move selection up/down".to_string());
            items.push("  Space — Select current item".to_string());
            items.push("  Enter or Tab — Continue".to_string());
            items.push("  Esc — Go back".to_string());
            items.push("  ? — Help".to_string());
            items.push("".to_string());
            push_options(
                &mut items,
                &app.locales,
                app.locale_index,
                Some(app.locale_selected),
            );
        }
        InstallStepType::Options => {
            items.push("⚙️ Installation options:".to_string());
            items.push("".to_string());
            items.push("⌨️ Keys:".to_string());
            items.push("  ↑/↓ or j/k — Move selection up/down".to_string());
            items.push("  Space — Toggle option on/off".to_string());
            items.push("  Enter or Tab — Continue".to_string());
            items.push("  Esc — Go back".to_string());
            items.push("  ? — Help".to_string());
            items.push("".to_string());
            let options = app
                .options
                .iter()
                .map(|option| {
                    format!(
                        "[{}] {}",
                        if option.enabled { "x" } else { " " },
                        option.label
                    )
                })
                .collect::<Vec<_>>();
            push_options(&mut items, &options, app.options_index, None);
        }
        InstallStepType::FirstBootUser => {
            items.push("🧑‍💻 First-boot user setup:".to_string());
            items.push("".to_string());
            items.push("⌨️ Keys:".to_string());
            items.push("  ↑/↓ or j/k — Move selection up/down".to_string());
            items.push("  Space — Select current item".to_string());
            items.push("  Enter or Tab — Continue".to_string());
            items.push("  Esc — Go back".to_string());
            items.push("  ? — Help".to_string());
            push_options(
                &mut items,
                &app.first_boot_options,
                app.first_boot_index,
                Some(app.first_boot_selected),
            );
        }
        InstallStepType::PlanReview => {
            items.push("🧾 Execution plan:".to_string());
            items.push("".to_string());
            items.push("Review exactly what will happen next:".to_string());
            for line in build_plan_lines(app) {
                items.push(line);
            }
            items.push("".to_string());
            if app.dry_run {
                items.push("🔧 Mode: DRY-RUN (no disk writes will occur)".to_string());
                items.push("".to_string());
            }
            items.push("⌨️ Keys:".to_string());
            items.push("  Enter or Tab — Continue".to_string());
            items.push("  Esc — Go back to modify settings".to_string());
            items.push("  ? — Help".to_string());
        }
        InstallStepType::Confirmation => {
            items.push("✅ Final confirmation:".to_string());
            items.push("".to_string());
            items.push("All settings reviewed. Ready to begin installation.".to_string());
            items.push("".to_string());
            if !app.dry_run && !app.destructive_armed {
                items.push(
                    "🛡️ SAFE MODE is ON — you'll be prompted to disarm before any disk writes."
                        .to_string(),
                );
                items.push("".to_string());
            }
            if !app.dry_run {
                items.push("⚠️ EXECUTE MODE — this will erase the selected disk.".to_string());
                items.push("".to_string());
            }
            items.push("⌨️ Keys:".to_string());
            items.push("  Enter or Tab — Continue".to_string());
            items.push("  Esc — Go back to review".to_string());
            items.push("  ? — Help".to_string());
            items.push("".to_string());
            let effective_image = app
                .downloaded_image_path
                .clone()
                .or_else(|| {
                    app.images
                        .get(app.image_index)
                        .map(|image| image.path.clone())
                })
                .unwrap_or_else(|| PathBuf::from(app.image_source_path.clone()));
            let download_efi = matches!(
                app.uefi_sources.get(app.uefi_source_selected),
                Some(flash_config::EfiSource::DownloadEfiImage)
            );
            let effective_efi = if download_efi {
                PathBuf::from("/tmp/mash-downloads/uefi")
            } else {
                PathBuf::from(app.uefi_source_path.clone())
            };
            if let Some(disk) = app.disks.get(app.disk_selected) {
                items.push(format!("Disk: {} ({})", disk.label, disk.path));
            }
            if let Some(distro) = app.os_distros.get(app.os_distro_selected) {
                items.push(format!("Distro: {}", distro.display()));
            }
            if let Some(variant) = app.os_variants.get(app.os_variant_selected) {
                items.push(format!("Flavour: {}", variant.label));
            }
            if let Some(scheme) = app.partition_schemes.get(app.scheme_selected) {
                items.push(format!("Scheme: {}", scheme));
            }
            if let Some(source) = app.image_sources.get(app.image_source_selected) {
                items.push(format!("Image source: {}", source.label));
            }
            items.push(format!("Image path: {}", effective_image.display()));
            if app.layout_selected < app.partition_layouts.len() {
                if let Some(layout) = app.partition_layouts.get(app.layout_selected) {
                    items.push(format!("Layout: {}", layout));
                }
            } else {
                items.push("Layout: Manual layout".to_string());
            }
            items.push(format!(
                "Partitions: EFI {} | BOOT {} | ROOT {} | DATA remainder",
                app.efi_size, app.boot_size, app.root_end
            ));
            if download_efi {
                items.push("EFI image: Download".to_string());
            } else {
                items.push("EFI image: Local".to_string());
            }
            items.push(format!("EFI path: {}", effective_efi.display()));
            if let Some(locale) = app.locales.get(app.locale_selected) {
                items.push(format!("Locale: {}", locale));
            }
            let download_image = app
                .image_sources
                .get(app.image_source_selected)
                .map(|source| source.value == flash_config::ImageSource::DownloadCatalogue)
                .unwrap_or(false);
            items.push(format!(
                "Downloads: Image={} | EFI={}",
                if download_image { "Yes" } else { "No" },
                if download_efi { "Yes" } else { "No" }
            ));
            if app.dry_run {
                items.push("Mode: DRY-RUN (no disk writes)".to_string());
            }
            items.push("Options:".to_string());
            for option in &app.options {
                items.push(format!(
                    "  - {}: {}",
                    option.label,
                    if option.enabled {
                        "Enabled"
                    } else {
                        "Disabled"
                    }
                ));
            }
            items.push(format!(
                "First boot: {}",
                app.first_boot_options
                    .get(app.first_boot_selected)
                    .cloned()
                    .unwrap_or_else(|| "Prompt to create user".to_string())
            ));
            push_options(&mut items, &["Go back".to_string()], 0, None);
            if let Some(error) = &app.error_message {
                items.push(format!("❌ {}", error));
            }
        }
        InstallStepType::ExecuteConfirmationGate => {
            items.push("🛑 FINAL EXECUTE GATE".to_string());
            items.push("".to_string());
            items.push("This is the last checkpoint before disk erase.".to_string());
            items.push("".to_string());
            items.push("Type EXACTLY (including spaces):".to_string());
            items.push("I UNDERSTAND THIS WILL ERASE THE SELECTED DISK".to_string());
            items.push("".to_string());
            items.push(format!("Input: {}", app.execute_confirmation_input));
            items.push("".to_string());
            items.push("⌨️ Keys:".to_string());
            items.push("  Type phrase — Input must match exactly".to_string());
            items.push("  Enter or Tab — Submit".to_string());
            items.push("  Esc — Cancel and go back".to_string());
            items.push("  ? — Help".to_string());
            items.push("".to_string());

            // Repeat the destructive intent summary.
            if let Some(distro) = app.os_distros.get(app.os_distro_selected) {
                items.push(format!("Distro: {}", distro.display()));
            }
            if let Some(variant) = app.os_variants.get(app.os_variant_selected) {
                items.push(format!("Flavour: {}", variant.label));
            }
            if let Some(disk) = app.disks.get(app.disk_selected) {
                items.push(format!("Disk: {} ({})", disk.label, disk.path));
            }
            if let Some(scheme) = app.partition_schemes.get(app.scheme_selected) {
                items.push(format!("Scheme: {}", scheme));
            }
            items.push(format!(
                "Partitions: EFI {} | BOOT {} | ROOT {} | DATA remainder",
                app.efi_size, app.boot_size, app.root_end
            ));
            let download_efi = matches!(
                app.uefi_sources.get(app.uefi_source_selected),
                Some(flash_config::EfiSource::DownloadEfiImage)
            );
            if download_efi {
                items.push("EFI image: Download".to_string());
            } else {
                items.push("EFI image: Local".to_string());
            }
            items.push(format!("EFI path: {}", app.uefi_source_path));

            if let Some(error) = &app.error_message {
                items.push(format!("❌ {}", error));
            }
        }
        InstallStepType::DisarmSafeMode => {
            items.push("🛡️ SAFE MODE is active.".to_string());
            items.push("You attempted a destructive action.".to_string());
            items.push("".to_string());
            items.push("⚠️ SAFE MODE ACTIVE — Disk writes are currently BLOCKED.".to_string());
            items.push("".to_string());
            items.push("To ARM the installer and enable destructive operations:".to_string());
            items.push("".to_string());
            items.push("⌨️ Keys:".to_string());
            items.push("  Type DESTROY (exactly) — Disarm safe mode and ARM installer".to_string());
            items.push("  Enter or Tab — Submit".to_string());
            items.push("  Esc — Cancel and go back".to_string());
            items.push("  ? — Help".to_string());
            items.push("".to_string());
            let (display, progress, total) =
                confirmation_progress("DESTROY", &app.safe_mode_disarm_input);
            items.push("Required: DESTROY".to_string());
            items.push(format!("Typed   : {}", display));
            items.push(format!("Progress: {}/{}", progress, total));
            if let Some(error) = &app.error_message {
                items.push(format!("❌ {}", error));
            }
            items.push("Esc cancels and returns to the summary.".to_string());
        }
        InstallStepType::DownloadingFedora => {
            let status = if app.downloaded_fedora {
                "✅ Fedora image downloaded (stub)."
            } else {
                "⬇️ Ready to simulate Fedora download."
            };
            items.push(status.to_string());
            items.push("".to_string());
            items.push("⌨️ Keys:".to_string());
            items.push("  ↑/↓ or j/k — Move selection up/down".to_string());
            items.push("  Space — Select current item".to_string());
            items.push("  Enter or Tab — Continue".to_string());
            items.push("  Esc — Go back".to_string());
            items.push("  ? — Help".to_string());
            push_options(
                &mut items,
                &[
                    "Mark Fedora download complete".to_string(),
                    "Go back".to_string(),
                ],
                app.downloading_fedora_index,
                None,
            );
        }
        InstallStepType::DownloadingUefi => {
            let status = if app.downloaded_uefi {
                "✅ EFI image downloaded (stub)."
            } else {
                "⬇️ Ready to simulate EFI download."
            };
            items.push(status.to_string());
            items.push("".to_string());
            items.push("⌨️ Keys:".to_string());
            items.push("  ↑/↓ or j/k — Move selection up/down".to_string());
            items.push("  Space — Select current item".to_string());
            items.push("  Enter or Tab — Continue".to_string());
            items.push("  Esc — Go back".to_string());
            items.push("  ? — Help".to_string());
            push_options(
                &mut items,
                &[
                    "Mark EFI download complete".to_string(),
                    "Go back".to_string(),
                ],
                app.downloading_uefi_index,
                None,
            );
        }
        InstallStepType::Flashing => {
            let spinner = spinner_frame(app.flash_start_time);
            let elapsed = elapsed_string(app.flash_start_time);
            let overall = progress_state.overall_percent.round().clamp(0.0, 100.0) as u16;
            let phase_percent = progress_state.phase_percent.round().clamp(0.0, 100.0) as u16;
            items.push(format!("{} Flashing in progress...", spinner));
            items.push(format!("Phase: {}", phase_hint(app)));
            items.push(format!("Elapsed: {}", elapsed));
            items.push(status_message(app, &progress_state));
            if app
                .cancel_requested
                .load(std::sync::atomic::Ordering::Relaxed)
                && !progress_state.is_complete
            {
                items.push("🛑 Cancel requested • Cleaning up...".to_string());
            }
            items.push(format!(
                "Overall: {}% • Step: {}% • ETA: {}",
                overall,
                phase_percent,
                progress_state.eta_string()
            ));
            items.extend(progress_phase_lines(&progress_state));
            items.push("".to_string());
            items.push("⌨️ Keys:".to_string());
            items.push("  Esc — Request cancellation (safe)".to_string());
            items.push("  Enter or Tab — Continue when complete".to_string());
            items.push("  ? — Help".to_string());
            push_options(&mut items, &["Viewing live telemetry".to_string()], 0, None);
        }
        InstallStepType::Complete => {
            if app.completion_lines.is_empty() {
                items.push("🎉 Installation complete.".to_string());
            } else {
                items.extend(app.completion_lines.clone());
            }
            items.push("".to_string());
            items.push("Press Enter or Tab to exit.".to_string());
            push_options(&mut items, &["Exit installer".to_string()], 0, None);
        }
    }

    if let Some(error) = &app.error_message {
        if matches!(
            app.current_step_type,
            InstallStepType::PartitionCustomize
                | InstallStepType::DiskConfirmation
                | InstallStepType::Confirmation
                | InstallStepType::ExecuteConfirmationGate
                | InstallStepType::DisarmSafeMode
        ) {
            return items;
        }
        items.push(format!("❌ {}", error));
    }

    items
}

fn confirmation_progress(required: &str, typed: &str) -> (String, usize, usize) {
    let total = required.chars().count();
    let typed_normalized: String = typed.chars().take(total).collect();
    let progress = typed_normalized.chars().count().min(total);
    let remaining = total.saturating_sub(progress);
    let mut display = typed_normalized;
    display.push_str(&"_".repeat(remaining));
    (display, progress, total)
}

fn push_options(
    items: &mut Vec<String>,
    options: &[String],
    cursor: usize,
    selected: Option<usize>,
) {
    if options.is_empty() {
        items.push("ℹ️ No options available.".to_string());
        return;
    }
    for (index, option) in options.iter().enumerate() {
        let cursor_marker = if index == cursor { "▶" } else { " " };
        let selected_marker = if selected == Some(index) { "*" } else { " " };
        items.push(format!("{}{} {}", cursor_marker, selected_marker, option));
    }
}

fn build_plan_lines(app: &App) -> Vec<String> {
    let mut lines = Vec::new();

    let distro = app.os_distros.get(app.os_distro_selected).copied();
    if let Some(distro) = distro {
        lines.push(format!("OS: {}", distro.display()));
    } else {
        lines.push("OS: <not selected>".to_string());
    }

    if let Some(variant) = app.os_variants.get(app.os_variant_selected) {
        lines.push(format!("Variant: {}", variant.label));
    } else {
        lines.push("Variant: <not selected>".to_string());
    }

    if let Some(disk) = app.disks.get(app.disk_selected) {
        lines.push(format!("Disk: {} ({})", disk.label, disk.path));
    } else {
        lines.push("Disk: <not selected>".to_string());
    }

    if let Some(source) = app.image_sources.get(app.image_source_selected) {
        match source.value {
            flash_config::ImageSource::DownloadCatalogue => {
                lines.push("Image: Download OS image".to_string());
            }
            flash_config::ImageSource::LocalFile => {
                lines.push(format!("Image: Local ({})", app.image_source_path));
            }
        }
    } else {
        lines.push("Image: <not selected>".to_string());
    }

    if let Some(scheme) = app.partition_schemes.get(app.scheme_selected) {
        lines.push(format!("Scheme: {}", scheme));
    } else {
        lines.push("Scheme: <not selected>".to_string());
    }

    if app.layout_selected < app.partition_layouts.len() {
        if let Some(layout) = app.partition_layouts.get(app.layout_selected) {
            lines.push(format!("Layout: {}", layout));
        }
    } else {
        lines.push("Layout: Manual".to_string());
    }
    lines.push(format!(
        "Partitions: EFI {} | BOOT {} | ROOT {} | DATA remainder",
        app.efi_size, app.boot_size, app.root_end
    ));

    if let Some(uefi_source) = app.uefi_sources.get(app.uefi_source_selected) {
        match uefi_source {
            flash_config::EfiSource::LocalEfiImage => {
                lines.push(format!("EFI: Local ({})", app.uefi_source_path));
            }
            flash_config::EfiSource::DownloadEfiImage => {
                lines.push("EFI: Download image".to_string());
            }
        }
    } else {
        lines.push("EFI: <not selected>".to_string());
    }

    if matches!(distro, Some(flash_config::OsDistro::Manjaro)) {
        lines.push("Note: post-boot partition expansion required".to_string());
    }

    lines.push("Reboots: 1".to_string());

    lines
}

pub(super) fn expected_actions(step: InstallStepType) -> String {
    let base = "↑/↓ or j/k: move  Space: select/toggle  Enter/Tab: continue  Esc: back  ?: help";
    match step {
        InstallStepType::DiskConfirmation => format!("{base}  |  Type: DESTROY"),
        InstallStepType::ExecuteConfirmationGate => {
            format!("{base}  |  Type: I UNDERSTAND THIS WILL ERASE THE SELECTED DISK")
        }
        InstallStepType::DisarmSafeMode => format!("{base}  |  Type: DESTROY"),
        InstallStepType::PartitionCustomize => {
            format!("{base}  |  Type: sizes  Backspace: delete  R: reset defaults")
        }
        _ => base.to_string(),
    }
}

pub(super) fn help_overlay_text(step: InstallStepType) -> String {
    // Must reflect global bindings exactly.
    let mut lines = Vec::new();
    lines.push("Contextual Help".to_string());
    lines.push("".to_string());
    lines.push(format!("Screen: {}", step.title()));
    lines.push("".to_string());
    lines.push("Navigation: Arrow Keys / j k  -> Move selection up/down".to_string());
    lines.push("Selection:  Space            -> Select / Toggle current item".to_string());
    lines
        .push("Continue:   Enter OR Tab     -> Continue to next screen / Confirm step".to_string());
    lines.push("Back/Quit:  Esc              -> Quit installer (or go back if safe)".to_string());
    lines.push("Help:       ?                -> Open contextual help overlay (modal)".to_string());
    lines.push("".to_string());

    match step {
        InstallStepType::DiskConfirmation => {
            lines.push("This step is destructive. Type the exact confirmation string.".to_string());
            lines.push("Required: DESTROY".to_string());
        }
        InstallStepType::ExecuteConfirmationGate => {
            lines.push("This step is destructive. Type the exact confirmation string.".to_string());
            lines.push("Required: I UNDERSTAND THIS WILL ERASE THE SELECTED DISK".to_string());
        }
        InstallStepType::DisarmSafeMode => {
            lines.push("Safe Mode is active. Disarm to enable disk writes.".to_string());
            lines.push("Required: DESTROY".to_string());
        }
        _ => {}
    }

    lines.push("".to_string());
    lines.push("Close: Esc or ?".to_string());
    lines.join("\n")
}

fn format_disk_entry(disk: &DiskOption) -> String {
    // Canonical label comes from HAL/sysfs (do not reconstruct identity in UI).
    let mut label = disk.label.clone();
    if disk.is_source_disk {
        label.push_str(" [SOURCE MEDIA]");
    }
    // Device path is useful for debugging, but should never be "device-first".
    format!("{} ({})", label, disk.path)
}

#[cfg(test)]
mod tests {
    use crate::dojo::dojo_ui::dump_step;

    #[test]
    fn plan_review_renders_summary() {
        let mut app = crate::dojo::dojo_app::App::new_with_flags(true);
        app.current_step_type = crate::dojo::dojo_app::InstallStepType::PlanReview;
        let dump = dump_step(&app);
        assert!(dump.contains("Execution plan"));
        assert!(dump.contains("Reboots"));
    }
}

pub(super) fn status_message(app: &App, progress_state: &ProgressState) -> String {
    let message = if !progress_state.status.is_empty() {
        progress_state.status.clone()
    } else {
        app.status_message.clone()
    };
    ensure_emoji_prefix(message)
}

fn format_partition_scheme(scheme: &mash_core::cli::PartitionScheme) -> String {
    match scheme {
        mash_core::cli::PartitionScheme::Mbr => "MBR — compatibility & simplicity".to_string(),
        mash_core::cli::PartitionScheme::Gpt => "GPT — modern, UEFI-oriented".to_string(),
    }
}

fn format_layout_preview(layout: &str) -> Vec<String> {
    layout
        .split('|')
        .map(|part| part.trim())
        .filter(|part| !part.is_empty())
        .map(|part| {
            let spaced = part.replace("MiB", " MiB").replace("GiB", " GiB");
            format!("  {}", spaced)
        })
        .collect()
}

fn spinner_frame(start: Option<std::time::Instant>) -> &'static str {
    let frames = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];
    let elapsed_ms = start
        .map(|instant| instant.elapsed().as_millis() as usize)
        .unwrap_or(0);
    let idx = (elapsed_ms / 100) % frames.len();
    frames[idx]
}

fn elapsed_string(start: Option<std::time::Instant>) -> String {
    let elapsed = start.map(|instant| instant.elapsed()).unwrap_or_default();
    let secs = elapsed.as_secs();
    let minutes = secs / 60;
    let seconds = secs % 60;
    format!("{:02}:{:02}", minutes, seconds)
}

fn phase_hint(app: &App) -> String {
    if let Some(phase) = app.progress_state_snapshot().current_phase {
        return phase.name().to_string();
    }
    let elapsed = app
        .flash_start_time
        .map(|instant| instant.elapsed().as_secs())
        .unwrap_or(0);
    match (elapsed / 5) % 3 {
        0 => "Writing image".to_string(),
        1 => "Syncing data".to_string(),
        _ => "Finalizing".to_string(),
    }
}

fn progress_phase_lines(progress_state: &ProgressState) -> Vec<String> {
    let mut lines = Vec::new();
    lines.push("Execution steps:".to_string());
    for phase in Phase::all() {
        let symbol = progress_state.phase_symbol(*phase);
        let marker = if progress_state.current_phase == Some(*phase) {
            "▶"
        } else {
            " "
        };
        lines.push(format!("{} {} {}", marker, symbol, phase.name()));
    }
    lines
}

fn ensure_emoji_prefix(message: String) -> String {
    match message.chars().next() {
        Some(first) if first.is_ascii_alphanumeric() => format!("ℹ️ {}", message),
        _ => message,
    }
}

pub(super) fn phase_line(progress_state: &ProgressState) -> String {
    match progress_state.current_phase {
        Some(phase) => {
            let phase_number = phase.number();
            let total = Phase::total();
            format!(
                "{} Phase {}/{}: {}",
                progress_state.phase_symbol(phase),
                phase_number,
                total,
                phase.name()
            )
        }
        None => "⏳ Phase: waiting for telemetry...".to_string(),
    }
}

pub(super) fn progress_detail(
    progress_state: &ProgressState,
    phase_line: &str,
    overall_line: &str,
    eta_line: &str,
) -> String {
    let speed_line = if progress_state.rsync_speed > 0.0 {
        format!("🚀 Speed: {:.1} MB/s", progress_state.rsync_speed)
    } else if progress_state.disk_io_speed > 0.0 {
        format!("💽 Disk: {:.1} MB/s", progress_state.disk_io_speed)
    } else {
        "💤 Speed: waiting...".to_string()
    };
    format!(
        "{}\n{} | {} | {}",
        phase_line, overall_line, eta_line, speed_line
    )
}
