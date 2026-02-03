#![allow(clippy::items_after_test_module)]
//! Dojo UI module for the single-screen TUI

use crate::tui::dojo_app::{App, InstallStepType};
use crate::tui::progress::{Phase, ProgressState};
use ratatui::{
    layout::{Constraint, Direction, Layout},
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Gauge, List, ListItem, Paragraph},
    Frame,
};
use std::path::PathBuf;

pub fn draw(f: &mut Frame, app: &App) {
    let progress_state = app.progress_state_snapshot();
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints(
            [
                Constraint::Length(3), // Title
                Constraint::Min(0),    // Steps
                Constraint::Length(5), // Progress block
                Constraint::Length(2), // Status line
            ]
            .as_ref(),
        )
        .split(f.area());

    // Title
    let (arming_label, arming_color) = if app.destructive_armed {
        ("ARMED", Color::Red)
    } else {
        ("SAFE", Color::Green)
    };
    let title_line = Line::from(vec![
        Span::styled("MASH Installer", Style::default().fg(Color::White)),
        Span::raw(" | "),
        Span::styled(arming_label, Style::default().fg(arming_color)),
    ]);
    let title = Block::default().borders(Borders::ALL).title(title_line);
    f.render_widget(title, chunks[0]);

    // Current Step Display
    let dojo_lines = build_dojo_lines(app);
    let list_items = dojo_lines
        .into_iter()
        .map(ListItem::new)
        .collect::<Vec<_>>();
    let list = List::new(list_items).block(Block::default().borders(Borders::ALL).title("Dojo"));
    f.render_widget(list, chunks[1]);

    // Progress bar
    let percent = progress_state.overall_percent.round().clamp(0.0, 100.0) as u16;
    let phase_line = phase_line(&progress_state);
    let eta_line = format!("⏱️ ETA: {}", progress_state.eta_string());
    let phase_percent = progress_state.phase_percent.round().clamp(0.0, 100.0) as u16;
    let overall_line = format!("📈 Overall: {}% | Phase: {}%", percent, phase_percent);
    let progress_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Length(2)].as_ref())
        .split(chunks[2]);
    let gauge = Gauge::default()
        .block(Block::default().borders(Borders::ALL).title("Progress"))
        .gauge_style(Style::default().fg(Color::Yellow))
        .percent(percent);
    f.render_widget(gauge, progress_chunks[0]);
    let progress_details = Paragraph::new(progress_detail(
        &progress_state,
        &phase_line,
        &overall_line,
        &eta_line,
    ))
    .block(Block::default().borders(Borders::ALL).title("Telemetry"));
    f.render_widget(progress_details, progress_chunks[1]);

    // Status line
    let status_message = status_message(app, &progress_state);
    let status = Paragraph::new(status_message)
        .block(Block::default().borders(Borders::ALL).title("Status"));
    f.render_widget(status, chunks[3]);
}

pub fn dump_step(app: &App) -> String {
    let progress_state = app.progress_state_snapshot();
    let dojo_lines = build_dojo_lines(app);
    let header = "MASH Installer";
    let dojo_hint = dojo_lines
        .first()
        .cloned()
        .unwrap_or_else(|| "🧭 Step: (unknown)".to_string());
    let body_lines = if dojo_lines.len() > 1 {
        dojo_lines[1..].join("\n")
    } else {
        "(no body content)".to_string()
    };
    let percent = progress_state.overall_percent.round().clamp(0.0, 100.0) as u16;
    let phase_line = phase_line(&progress_state);
    let eta_line = format!("⏱️ ETA: {}", progress_state.eta_string());
    let phase_percent = progress_state.phase_percent.round().clamp(0.0, 100.0) as u16;
    let overall_line = format!("📈 Overall: {}% | Phase: {}%", percent, phase_percent);
    let telemetry = progress_detail(&progress_state, &phase_line, &overall_line, &eta_line);
    let status = status_message(app, &progress_state);
    let actions = expected_actions(app.current_step_type);

    format!(
        "STEP: {}\n\n- Header: {}\n- Dojo hint line: {}\n- Body contents:\n{}\n- Footer/progress/telemetry/status blocks:\nProgress: {}%\nTelemetry: {}\nStatus: {}\n- Expected user actions (keys): {}\n",
        app.current_step_type.title(),
        header,
        dojo_hint,
        body_lines,
        percent,
        telemetry,
        status,
        actions
    )
}

fn build_dojo_lines(app: &App) -> Vec<String> {
    let current_step_title = app.current_step_type.title();
    let mut items = Vec::new();
    items.push(format!("🧭 Step: {}", current_step_title));
    let progress_state = app.progress_state_snapshot();

    match app.current_step_type {
        InstallStepType::Welcome => {
            items.push("👋 Welcome to MASH: a safe, guided installer.".to_string());
            items.push("🛡️ No disks will be modified unless the installer is ARMED.".to_string());
            items.push("⌨️ Enter to proceed • Esc to go back • q to quit.".to_string());
            items.push(format!(
                "Arming state: {}",
                if app.destructive_armed {
                    "ARMED (writes enabled)"
                } else {
                    "SAFE (disarmed)"
                }
            ));
            push_options(&mut items, &app.welcome_options, app.welcome_index);
        }
        InstallStepType::DiskSelection => {
            items.push("💽 Select a target disk:".to_string());
            items
                .push("Use ↑/↓ or Tab to choose • Enter to continue • Esc to go back.".to_string());

            // Show warning banner if boot detection failed
            use crate::tui::data_sources::BootConfidence;
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
            push_options(&mut items, &options, app.disk_index);

            // Show boot disk override instructions if boot disk is selected
            if let Some(disk) = app.disks.get(app.disk_index) {
                if disk.boot_confidence.is_boot() {
                    items.push("".to_string());
                    items.push("⚠️ DANGER: Boot disk selected!".to_string());
                    items.push(
                        "Type 'BOOT' to override protection, or select a different disk."
                            .to_string(),
                    );
                    if !app.boot_disk_override_input.is_empty() {
                        items.push(format!("Input: {}", app.boot_disk_override_input));
                    }
                }
            }
        }
        InstallStepType::DiskConfirmation => {
            let disk = app.disks.get(app.disk_index);
            if let Some(disk) = disk {
                let is_boot_disk = disk.boot_confidence.is_boot();
                let model = disk.model.as_deref().unwrap_or("Unknown model").trim();

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
                        "BOOT DEVICE TO BE WIPED: {} ({}, {})",
                        disk.path, model, disk.size
                    ));
                    items.push("".to_string());
                    items.push("Type 'DESTROY BOOT DISK' to confirm • Esc to go back.".to_string());
                } else {
                    items.push("⚠️ Confirm disk destruction:".to_string());
                    items.push("Type DESTROY to confirm • Esc to go back.".to_string());
                    items.push(format!(
                        "TARGET TO BE WIPED: {} ({}, {})",
                        disk.path, model, disk.size
                    ));
                    items.push("Type DESTROY to confirm.".to_string());
                }

                items.push(format!("Input: {}", app.wipe_confirmation));
            } else {
                items.push("No disk selected.".to_string());
            }
            if let Some(error) = &app.error_message {
                items.push(format!("❌ {}", error));
            }
        }
        InstallStepType::BackupConfirmation => {
            items.push("⚠️ This will erase data on the selected disk.".to_string());
            items.push("💾 Have you backed up your data? (Y/N)".to_string());
            if app.backup_confirmed {
                items.push("✅ Backup confirmed.".to_string());
            }
            push_options(
                &mut items,
                &[
                    "No, go back".to_string(),
                    "Yes, I have a backup".to_string(),
                ],
                app.backup_choice_index,
            );
        }
        InstallStepType::PartitionScheme => {
            items.push("🧩 Select a partition scheme:".to_string());
            items
                .push("Use ↑/↓ or Tab to choose • Enter to continue • Esc to go back.".to_string());
            let options = app
                .partition_schemes
                .iter()
                .map(format_partition_scheme)
                .collect::<Vec<_>>();
            push_options(&mut items, &options, app.scheme_index);
        }
        InstallStepType::PartitionLayout => {
            items.push("📐 Select a partition layout:".to_string());
            items.push(
                "Use ↑/↓ or Tab to choose • Y to accept • N to customize • Esc to go back."
                    .to_string(),
            );
            let layout_options = app
                .partition_layouts
                .iter()
                .enumerate()
                .map(|(idx, _)| format!("Layout {}", idx + 1))
                .collect::<Vec<_>>();
            push_options(&mut items, &layout_options, app.layout_index);
            if let Some(layout) = app.partition_layouts.get(app.layout_index) {
                items.push("Preview:".to_string());
                items.extend(format_layout_preview(layout));
            }
        }
        InstallStepType::PartitionCustomize => {
            items.push("🛠️ Customize partitions:".to_string());
            items.push(
                "Use Tab/↑/↓ to select • Type to edit • Backspace to delete • R to reset • Enter."
                    .to_string(),
            );
            let options = app
                .partition_customizations
                .iter()
                .enumerate()
                .map(|(idx, option)| {
                    let field = match idx {
                        0 => Some(crate::tui::dojo_app::CustomizeField::Efi),
                        1 => Some(crate::tui::dojo_app::CustomizeField::Boot),
                        2 => Some(crate::tui::dojo_app::CustomizeField::Root),
                        _ => None,
                    };
                    if field.is_some() && app.customize_error_field == field {
                        format!("❌ {}", option)
                    } else {
                        option.clone()
                    }
                })
                .collect::<Vec<_>>();
            push_options(&mut items, &options, app.customize_index);
            if let Some(error) = &app.error_message {
                items.push(format!("❌ {}", error));
            }
        }
        InstallStepType::DownloadSourceSelection => {
            items.push("📥 Select image source:".to_string());
            items
                .push("Use ↑/↓ or Tab to choose • Enter to continue • Esc to go back.".to_string());
            let options = app
                .image_sources
                .iter()
                .map(|source| source.label.clone())
                .collect::<Vec<_>>();
            push_options(&mut items, &options, app.image_source_index);
            if app
                .image_sources
                .get(app.image_source_index)
                .map(|source| source.value == crate::tui::flash_config::ImageSource::LocalFile)
                .unwrap_or(false)
            {
                items.push("Local image path:".to_string());
                items.push(app.image_source_path.clone());
            }
        }
        InstallStepType::ImageSelection => {
            items.push("🖼️ Select OS distribution:".to_string());
            items
                .push("Use ↑/↓ or Tab to choose • Enter to continue • Esc to go back.".to_string());

            // Show OS distro options
            let options = app
                .os_distros
                .iter()
                .map(|distro| distro.display().to_string())
                .collect::<Vec<_>>();
            push_options(&mut items, &options, app.os_distro_index);

            // Show availability warning for non-Fedora distros
            if let Some(distro) = app.os_distros.get(app.os_distro_index) {
                if !distro.is_available() {
                    items.push("".to_string());
                    items.push("⚠️ This distribution is not yet available.".to_string());
                    items.push("Please select Fedora for now.".to_string());
                }
            }
        }
        InstallStepType::EfiImage => {
            items.push("🧩 Choose how to get the EFI image:".to_string());
            items
                .push("Use ↑/↓ or Tab to choose • Enter to continue • Esc to go back.".to_string());

            let uefi_source = app.uefi_sources.get(app.uefi_source_index);
            let is_local = matches!(
                uefi_source,
                Some(crate::tui::flash_config::EfiSource::LocalEfiImage)
            );

            // Show EFI source options (intent-only)
            let options = app
                .uefi_sources
                .iter()
                .map(|source| source.display().to_string())
                .collect::<Vec<_>>();
            push_options(&mut items, &options, app.uefi_source_index);

            // Only ask for a path if the user chose "Use local EFI image".
            if is_local {
                items.push("".to_string());
                items.push("Local EFI image path:".to_string());
                items.push(app.uefi_source_path.clone());
            }
        }
        InstallStepType::LocaleSelection => {
            items.push("🗣️ Select locale and keymap:".to_string());
            items
                .push("Use ↑/↓ or Tab to choose • Enter to continue • Esc to go back.".to_string());
            push_options(&mut items, &app.locales, app.locale_index);
        }
        InstallStepType::Options => {
            items.push("⚙️ Installation options:".to_string());
            items.push("Use ↑/↓ to focus • Space/Enter to toggle • Esc to go back.".to_string());
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
            push_options(&mut items, &options, app.options_index);
        }
        InstallStepType::FirstBootUser => {
            items.push("🧑‍💻 First-boot user setup:".to_string());
            items
                .push("Use ↑/↓ or Tab to choose • Enter to continue • Esc to go back.".to_string());
            push_options(&mut items, &app.first_boot_options, app.first_boot_index);
        }
        InstallStepType::PlanReview => {
            items.push("🧾 Execution plan:".to_string());
            items.push("Review exactly what will happen next.".to_string());
            for line in build_plan_lines(app) {
                items.push(line);
            }
            if app.dry_run {
                items.push("Mode: DRY-RUN (no disk writes)".to_string());
            }
            items.push("Press Enter to proceed to confirmation • Esc to go back.".to_string());
        }
        InstallStepType::Confirmation => {
            items.push("✅ Review configuration summary:".to_string());
            items.push("Press Enter to begin installation.".to_string());
            if !app.dry_run && !app.destructive_armed {
                items.push(
                    "SAFE MODE is ON — you'll be prompted to disarm before any disk writes."
                        .to_string(),
                );
            }
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
                app.uefi_sources.get(app.uefi_source_index),
                Some(crate::tui::flash_config::EfiSource::DownloadEfiImage)
            );
            let effective_efi = if download_efi {
                PathBuf::from("/tmp/mash-downloads/uefi")
            } else {
                PathBuf::from(app.uefi_source_path.clone())
            };
            if let Some(disk) = app.disks.get(app.disk_index) {
                items.push(format!("Disk: {} ({})", disk.path, disk.size));
                items.push(format!("Disk label: {}", disk.label));
            }
            if let Some(scheme) = app.partition_schemes.get(app.scheme_index) {
                items.push(format!("Scheme: {}", scheme));
            }
            if let Some(source) = app.image_sources.get(app.image_source_index) {
                items.push(format!("Image source: {}", source.label));
            }
            items.push(format!("Image path: {}", effective_image.display()));
            if let Some(layout) = app.partition_layouts.get(app.layout_index) {
                items.push(format!("Layout: {}", layout));
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
            if let Some(locale) = app.locales.get(app.locale_index) {
                items.push(format!("Locale: {}", locale));
            }
            let download_fedora = app
                .options
                .iter()
                .find(|option| option.label == "Download Fedora image")
                .map(|option| option.enabled)
                .unwrap_or(false);
            items.push(format!(
                "Downloads: Fedora={} | EFI={}",
                if download_fedora { "Yes" } else { "No" },
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
                    .get(app.first_boot_index)
                    .cloned()
                    .unwrap_or_else(|| "Prompt to create user".to_string())
            ));
            push_options(&mut items, &["Go back".to_string()], 0);
            if let Some(error) = &app.error_message {
                items.push(format!("❌ {}", error));
            }
        }
        InstallStepType::DisarmSafeMode => {
            items.push("🛡️ SAFE MODE is active.".to_string());
            items.push("You attempted a destructive action.".to_string());
            items.push("".to_string());
            items.push("To DISARM SAFE MODE and enable disk writes, type: DESTROY".to_string());
            items.push("Then press Enter.".to_string());
            items.push("".to_string());
            items.push(format!("Input: {}", app.safe_mode_disarm_input));
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
            items.push("Use ↑/↓ to choose • Enter to continue • Esc to go back.".to_string());
            push_options(
                &mut items,
                &[
                    "Mark Fedora download complete".to_string(),
                    "Go back".to_string(),
                ],
                app.downloading_fedora_index,
            );
        }
        InstallStepType::DownloadingUefi => {
            let status = if app.downloaded_uefi {
                "✅ EFI image downloaded (stub)."
            } else {
                "⬇️ Ready to simulate EFI download."
            };
            items.push(status.to_string());
            items.push("Use ↑/↓ to choose • Enter to continue • Esc to go back.".to_string());
            push_options(
                &mut items,
                &[
                    "Mark EFI download complete".to_string(),
                    "Go back".to_string(),
                ],
                app.downloading_uefi_index,
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
            push_options(&mut items, &["Viewing live telemetry".to_string()], 0);
        }
        InstallStepType::Complete => {
            items.push("🎉 Installation complete.".to_string());
            items.push("Press Enter to exit.".to_string());
            push_options(&mut items, &["Exit installer".to_string()], 0);
        }
    }

    if let Some(error) = &app.error_message {
        if matches!(
            app.current_step_type,
            InstallStepType::PartitionCustomize
                | InstallStepType::DiskConfirmation
                | InstallStepType::Confirmation
                | InstallStepType::DisarmSafeMode
        ) {
            return items;
        }
        items.push(format!("❌ {}", error));
    }

    items
}

fn push_options(items: &mut Vec<String>, options: &[String], selected: usize) {
    if options.is_empty() {
        items.push("ℹ️ No options available.".to_string());
        return;
    }
    for (index, option) in options.iter().enumerate() {
        let marker = if index == selected { "▶" } else { " " };
        items.push(format!("{} {}", marker, option));
    }
}

fn build_plan_lines(app: &crate::tui::dojo_app::App) -> Vec<String> {
    let mut lines = Vec::new();

    // OS Distribution
    if let Some(distro) = app.os_distros.get(app.os_distro_index) {
        lines.push(format!("OS: {}", distro.display()));
    } else {
        lines.push("OS: <not selected>".to_string());
    }

    // Target disk
    if let Some(disk) = app.disks.get(app.disk_index) {
        lines.push(format!("Disk: {} ({})", disk.path, disk.size));
    } else {
        lines.push("Disk: <not selected>".to_string());
    }

    // EFI image source
    if let Some(uefi_source) = app.uefi_sources.get(app.uefi_source_index) {
        match uefi_source {
            crate::tui::flash_config::EfiSource::LocalEfiImage => {
                lines.push(format!("EFI: Local ({})", app.uefi_source_path));
            }
            crate::tui::flash_config::EfiSource::DownloadEfiImage => {
                lines.push("EFI: Download image".to_string());
            }
        }
    } else {
        lines.push("EFI: <not selected>".to_string());
    }

    lines.push("Mounts: /boot/efi, /boot, / (root), /data".to_string());
    lines.push("Formats: EFI=fat32, BOOT=ext4, ROOT=btrfs, DATA=btrfs".to_string());
    lines.push("Packages: (not selected)".to_string());
    lines.push("Kernel fix: USB-root alignment check".to_string());
    lines.push("Reboots: 1".to_string());
    lines
}

fn expected_actions(step: InstallStepType) -> String {
    match step {
        InstallStepType::BackupConfirmation => "Up/Down, Y/N, Enter, Esc, q".to_string(),
        InstallStepType::Flashing => "Enter when complete, C to cancel, q".to_string(),
        InstallStepType::Complete => "Enter to exit".to_string(),
        InstallStepType::DiskConfirmation => "Type DESTROY, Enter, Esc, q".to_string(),
        InstallStepType::DownloadingFedora | InstallStepType::DownloadingUefi => {
            "Up/Down, Enter, Esc, q".to_string()
        }
        InstallStepType::Options => "Up/Down, Space/Enter, Esc, q".to_string(),
        InstallStepType::Welcome => "Up/Down, Enter, q".to_string(),
        InstallStepType::PartitionLayout => "Up/Down/Tab, Y/N, Enter, Esc, q".to_string(),
        InstallStepType::PartitionScheme => "Up/Down/Tab, Enter, Esc, q".to_string(),
        InstallStepType::PartitionCustomize => {
            "Up/Down/Tab, Type, Backspace, R, Enter, Esc, q".to_string()
        }
        InstallStepType::PlanReview => "Enter, Esc, q".to_string(),
        InstallStepType::Confirmation => "Enter, Esc, q".to_string(),
        InstallStepType::DisarmSafeMode => "Type DESTROY, Enter, Esc, q".to_string(),
        InstallStepType::DiskSelection
        | InstallStepType::ImageSelection
        | InstallStepType::LocaleSelection
        | InstallStepType::FirstBootUser => "Up/Down/Tab, Enter, Esc, q".to_string(),
        InstallStepType::DownloadSourceSelection | InstallStepType::EfiImage => {
            "Up/Down/Tab, Type, Backspace, Enter, Esc, q".to_string()
        }
    }
}

fn format_disk_entry(disk: &crate::tui::dojo_app::DiskOption) -> String {
    use crate::tui::data_sources::BootConfidence;

    // Build human-readable identity from label (which already includes size + vendor/model)
    let mut identity = disk.label.clone();

    // Add transport hint if available
    let transport_hint = disk.transport.hint();
    if !transport_hint.is_empty() {
        identity.push_str(&format!(" ({})", transport_hint));
    }

    // Build tags
    let mut tags = Vec::new();

    // Boot confidence tags
    match disk.boot_confidence {
        BootConfidence::Confident => tags.push("⚠ BOOT MEDIA"),
        BootConfidence::Unverified => tags.push("⚠ BOOT?"),
        BootConfidence::NotBoot | BootConfidence::Unknown => {
            // Show removable/internal for non-boot disks
            if disk.removable {
                tags.push("REMOVABLE");
            } else {
                tags.push("INTERNAL");
            }
        }
    }

    let tag_str = if tags.is_empty() {
        String::new()
    } else {
        format!(" - {}", tags.join(", "))
    };

    format!("{} - {}{}", disk.path, identity, tag_str)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plan_review_renders_summary() {
        let mut app = crate::tui::dojo_app::App::new_with_flags(true);
        app.current_step_type = crate::tui::dojo_app::InstallStepType::PlanReview;
        let dump = dump_step(&app);
        assert!(dump.contains("Execution plan"));
        assert!(dump.contains("Reboots"));
    }
}

fn status_message(app: &App, progress_state: &ProgressState) -> String {
    let message = if !progress_state.status.is_empty() {
        progress_state.status.clone()
    } else {
        app.status_message.clone()
    };
    ensure_emoji_prefix(message)
}

fn format_partition_scheme(scheme: &crate::cli::PartitionScheme) -> String {
    match scheme {
        crate::cli::PartitionScheme::Mbr => "MBR — compatibility & simplicity".to_string(),
        crate::cli::PartitionScheme::Gpt => "GPT — modern, UEFI-oriented".to_string(),
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

fn phase_line(progress_state: &ProgressState) -> String {
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

fn progress_detail(
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
