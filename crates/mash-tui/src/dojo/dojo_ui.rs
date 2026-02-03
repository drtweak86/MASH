#![allow(clippy::items_after_test_module)]
//! Dojo UI module for the single-screen TUI

use super::dojo_app::{App, InstallStepType};
use crate::progress::{Phase, ProgressState};
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

    // Main layout: Title | Main Body | Progress Bar | Key Legend
    let main_chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints(
            [
                Constraint::Length(3), // Title bar
                Constraint::Min(10),   // Main body (3-panel)
                Constraint::Length(3), // Progress bar
                Constraint::Length(3), // Key legend
            ]
            .as_ref(),
        )
        .split(f.area());

    // Title bar: mode + SAFE/ARMED indicator (must be unambiguous).
    let (mode_label, mode_color) = if app.dry_run {
        ("DRY-RUN", Color::Yellow)
    } else {
        ("EXECUTE", Color::Red)
    };
    let (arming_label, arming_color) = if app.destructive_armed {
        ("ARMED", Color::Red)
    } else {
        ("SAFE", Color::Green)
    };
    let title_line = Line::from(vec![
        Span::styled("MASH Installer", Style::default().fg(Color::White)),
        Span::raw(" | "),
        Span::styled(mode_label, Style::default().fg(mode_color)),
        Span::raw(" | "),
        Span::styled(arming_label, Style::default().fg(arming_color)),
    ]);
    let title = Block::default().borders(Borders::ALL).title(title_line);
    f.render_widget(title, main_chunks[0]);

    // Three-panel layout: Sidebar | Content | Info Panel
    let body_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints(
            [
                Constraint::Percentage(20), // Left: Step sidebar
                Constraint::Percentage(55), // Center: Main content
                Constraint::Percentage(25), // Right: Info panel
            ]
            .as_ref(),
        )
        .split(main_chunks[1]);

    // Left sidebar: Step progress
    let sidebar_content = build_step_sidebar(app);
    let sidebar = Paragraph::new(sidebar_content)
        .block(Block::default().borders(Borders::ALL).title("Steps"));
    f.render_widget(sidebar, body_chunks[0]);

    // Center: Main content (current step)
    let dojo_lines = build_dojo_lines(app);
    let list_items = dojo_lines
        .into_iter()
        .map(ListItem::new)
        .collect::<Vec<_>>();
    let content = List::new(list_items).block(
        Block::default()
            .borders(Borders::ALL)
            .title(app.current_step_type.title()),
    );
    f.render_widget(content, body_chunks[1]);

    // Right panel: Info summary
    let info_content = build_info_panel(app, &progress_state);
    let info_panel =
        Paragraph::new(info_content).block(Block::default().borders(Borders::ALL).title("Info"));
    f.render_widget(info_panel, body_chunks[2]);

    // Progress bar
    let percent = progress_state.overall_percent.round().clamp(0.0, 100.0) as u16;
    let gauge = Gauge::default()
        .block(Block::default().borders(Borders::ALL).title("Progress"))
        .gauge_style(Style::default().fg(Color::Yellow))
        .percent(percent);
    f.render_widget(gauge, main_chunks[2]);

    // Key legend (always visible, context-specific)
    let key_help = expected_actions(app.current_step_type);
    let status_msg = status_message(app, &progress_state);
    let legend_text = format!("{}\n{}", status_msg, key_help);
    let legend =
        Paragraph::new(legend_text).block(Block::default().borders(Borders::ALL).title("Keys"));
    f.render_widget(legend, main_chunks[3]);
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

/// Build the left sidebar showing step progression
fn build_step_sidebar(app: &App) -> String {
    let all_steps = [
        (InstallStepType::Welcome, "Welcome"),
        (InstallStepType::ImageSelection, "Select OS"),
        (InstallStepType::VariantSelection, "Select Flavour"),
        (InstallStepType::DownloadSourceSelection, "Image Source"),
        (InstallStepType::DiskSelection, "Select Disk"),
        (InstallStepType::DiskConfirmation, "Confirm Disk"),
        (InstallStepType::PartitionScheme, "Partition Scheme"),
        (InstallStepType::PartitionLayout, "Partition Layout"),
        (InstallStepType::EfiImage, "EFI Setup"),
        (InstallStepType::LocaleSelection, "Locale/Keymap"),
        (InstallStepType::Options, "Options"),
        (InstallStepType::PlanReview, "Review Plan"),
        (InstallStepType::Confirmation, "Confirm"),
        (InstallStepType::ExecuteConfirmationGate, "Execute Gate"),
        (InstallStepType::Flashing, "Installing..."),
        (InstallStepType::Complete, "Complete!"),
    ];

    let mut lines = Vec::new();
    for (step, label) in &all_steps {
        let marker = if *step == app.current_step_type {
            "▶"
        } else if is_step_before(*step, app.current_step_type) {
            "✓"
        } else {
            " "
        };
        lines.push(format!("{} {}", marker, label));
    }
    lines.join("\n")
}

/// Check if step_a comes before step_b in the flow
fn is_step_before(step_a: InstallStepType, step_b: InstallStepType) -> bool {
    let order = [
        InstallStepType::Welcome,
        InstallStepType::ImageSelection,
        InstallStepType::VariantSelection,
        InstallStepType::DownloadSourceSelection,
        InstallStepType::DiskSelection,
        InstallStepType::DiskConfirmation,
        InstallStepType::PartitionScheme,
        InstallStepType::PartitionLayout,
        InstallStepType::PartitionCustomize,
        InstallStepType::EfiImage,
        InstallStepType::LocaleSelection,
        InstallStepType::Options,
        InstallStepType::PlanReview,
        InstallStepType::Confirmation,
        InstallStepType::ExecuteConfirmationGate,
        InstallStepType::DisarmSafeMode,
        InstallStepType::Flashing,
        InstallStepType::Complete,
    ];

    let pos_a = order.iter().position(|&s| s == step_a);
    let pos_b = order.iter().position(|&s| s == step_b);

    match (pos_a, pos_b) {
        (Some(a), Some(b)) => a < b,
        _ => false,
    }
}

/// Build the right info panel showing current selections and status
fn build_info_panel(app: &App, progress_state: &ProgressState) -> String {
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
    if let Some(distro) = app.os_distros.get(app.os_distro_index) {
        lines.push(format!("OS: {}", distro.display()));
    }

    // Variant selection
    if let Some(variant) = app.os_variants.get(app.os_variant_index) {
        lines.push(format!("Variant: {}", variant.label));
    }

    // Disk selection
    if let Some(disk) = app.disks.get(app.disk_index) {
        if let Some(ref identity) = disk.identity {
            lines.push(format!("Disk: {}", identity.display_string()));
        } else {
            lines.push(format!("Disk: {}", disk.path));
        }
    }

    // Partition scheme
    if let Some(scheme) = app.partition_schemes.get(app.scheme_index) {
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

fn build_dojo_lines(app: &App) -> Vec<String> {
    let current_step_title = app.current_step_type.title();
    let mut items = Vec::new();
    items.push(format!("🧭 Step: {}", current_step_title));
    let progress_state = app.progress_state_snapshot();

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
            items.push("  Enter — Proceed to OS selection".to_string());
            items.push("  q — Quit installer".to_string());
            push_options(&mut items, &app.welcome_options, app.welcome_index);
        }
        InstallStepType::DiskSelection => {
            items.push("💽 Select a target disk:".to_string());
            items.push("".to_string());
            items.push("⌨️ Keys:".to_string());
            items.push("  ↑/↓ or Tab — Move selection up/down".to_string());
            items.push("  Enter — Confirm disk choice".to_string());
            items.push("  r — Refresh disk list".to_string());
            items.push("  Esc — Go back to previous step".to_string());
            items.push("  q — Quit installer".to_string());
            items.push("".to_string());

            // Show warning banner if boot detection failed
            use super::data_sources::BootConfidence;
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
            let disk = app.disks.get(app.disk_index);
            if let Some(disk) = disk {
                let is_boot_disk = disk.boot_confidence.is_boot() || disk.is_source_disk;
                let disk_info = match &disk.identity {
                    Some(identity) => identity.display_string(),
                    None => {
                        items.push("❌ IDENTITY FAILED - Cannot proceed".to_string());
                        items.push("Disk identity could not be resolved.".to_string());
                        items.push("Press Esc to go back and select a different disk.".to_string());
                        return items;
                    }
                };

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
                    items.push("Type 'DESTROY BOOT DISK' to confirm • Esc to go back.".to_string());
                } else {
                    items.push("⚠️ Confirm disk destruction:".to_string());
                    items.push("".to_string());
                    items.push(format!("TARGET TO BE WIPED: {} ({})", disk.path, disk_info));
                    items.push("".to_string());
                    items.push("⌨️ Keys:".to_string());
                    items.push("  Type DESTROY (exactly) — Confirm and proceed".to_string());
                    items.push("  Esc — Cancel and go back".to_string());
                    items.push("".to_string());
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
            items.push("".to_string());
            items.push("MBR: Compatible with older systems, simpler structure".to_string());
            items.push("GPT: Modern standard, supports larger disks, UEFI-oriented".to_string());
            items.push("".to_string());
            items.push("⌨️ Keys:".to_string());
            items.push("  ↑/↓ or Tab — Switch between MBR and GPT".to_string());
            items.push("  Enter — Confirm choice".to_string());
            items.push("  Esc — Go back".to_string());
            let options = app
                .partition_schemes
                .iter()
                .map(format_partition_scheme)
                .collect::<Vec<_>>();
            push_options(&mut items, &options, app.scheme_index);
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

            push_options(&mut items, &layout_options, app.layout_index);

            // Show detailed preview of selected layout
            if let Some(layout) = app.partition_layouts.get(app.layout_index) {
                items.push("".to_string());
                items.push("Partition details:".to_string());
                items.extend(format_layout_preview(layout));
            }

            items.push("".to_string());
            items.push("⌨️ ↑/↓ or Tab to choose • Enter or Y to accept • M to customize manually • Esc to go back".to_string());
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
                        0 => Some(super::dojo_app::CustomizeField::Efi),
                        1 => Some(super::dojo_app::CustomizeField::Boot),
                        2 => Some(super::dojo_app::CustomizeField::Root),
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
                .map(|source| source.value == super::flash_config::ImageSource::LocalFile)
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
            items.push("  ↑/↓ or Tab — Move between OS options".to_string());
            items.push("  Enter — Confirm OS choice".to_string());
            items.push("  Esc — Go back".to_string());
            items.push("".to_string());

            // Show OS distro options
            let options = app
                .os_distros
                .iter()
                .map(|distro| distro.display().to_string())
                .collect::<Vec<_>>();
            push_options(&mut items, &options, app.os_distro_index);
            items.push("".to_string());
            items.push("Next: pick a variant (server/desktop/etc).".to_string());
        }
        InstallStepType::VariantSelection => {
            items.push("🎛️ Select OS flavour/variant:".to_string());
            items.push("".to_string());
            items.push("Choose the edition or desktop environment for your OS.".to_string());
            items.push("".to_string());
            items.push("⌨️ Keys:".to_string());
            items.push("  ↑/↓ or Tab — Move between variants".to_string());
            items.push("  Enter — Confirm variant choice".to_string());
            items.push("  Esc — Go back to OS selection".to_string());
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
                push_options(&mut items, &options, app.os_variant_index);
            }
        }
        InstallStepType::EfiImage => {
            items.push("🧩 Choose how to get the EFI image:".to_string());
            items
                .push("Use ↑/↓ or Tab to choose • Enter to continue • Esc to go back.".to_string());

            let uefi_source = app.uefi_sources.get(app.uefi_source_index);
            let is_local = matches!(
                uefi_source,
                Some(super::flash_config::EfiSource::LocalEfiImage)
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
            items.push("".to_string());
            items.push("⌨️ Keys:".to_string());
            items.push("  ↑/↓ or Tab — Browse locale options".to_string());
            items.push("  Enter — Confirm locale choice".to_string());
            items.push("  Esc — Go back".to_string());
            items.push("".to_string());
            push_options(&mut items, &app.locales, app.locale_index);
        }
        InstallStepType::Options => {
            items.push("⚙️ Installation options:".to_string());
            items.push("".to_string());
            items.push("⌨️ Keys:".to_string());
            items.push("  ↑/↓ — Move between options".to_string());
            items.push("  Space or Enter — Toggle option on/off".to_string());
            items.push("  Enter (when done) — Proceed to review".to_string());
            items.push("  Esc — Go back".to_string());
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
            items.push("  Enter — Proceed to final confirmation".to_string());
            items.push("  Esc — Go back to modify settings".to_string());
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
            items.push("  Enter — Continue".to_string());
            items.push("  Esc — Go back to review".to_string());
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
                app.uefi_sources.get(app.uefi_source_index),
                Some(super::flash_config::EfiSource::DownloadEfiImage)
            );
            let effective_efi = if download_efi {
                PathBuf::from("/tmp/mash-downloads/uefi")
            } else {
                PathBuf::from(app.uefi_source_path.clone())
            };
            if let Some(disk) = app.disks.get(app.disk_index) {
                let disk_info = match &disk.identity {
                    Some(identity) => identity.display_string(),
                    None => "❌ IDENTITY FAILED".to_string(),
                };
                items.push(format!("Disk: {} - {}", disk.path, disk_info));
            }
            if let Some(distro) = app.os_distros.get(app.os_distro_index) {
                items.push(format!("Distro: {}", distro.display()));
            }
            if let Some(variant) = app.os_variants.get(app.os_variant_index) {
                items.push(format!("Flavour: {}", variant.label));
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
            items.push("  Enter — Submit".to_string());
            items.push("  Esc — Cancel and go back".to_string());
            items.push("".to_string());

            // Repeat the destructive intent summary.
            if let Some(distro) = app.os_distros.get(app.os_distro_index) {
                items.push(format!("Distro: {}", distro.display()));
            }
            if let Some(variant) = app.os_variants.get(app.os_variant_index) {
                items.push(format!("Flavour: {}", variant.label));
            }
            if let Some(disk) = app.disks.get(app.disk_index) {
                let disk_info = match &disk.identity {
                    Some(identity) => identity.display_string(),
                    None => "❌ IDENTITY FAILED".to_string(),
                };
                items.push(format!("Disk: {} - {}", disk.path, disk_info));
            }
            if let Some(scheme) = app.partition_schemes.get(app.scheme_index) {
                items.push(format!("Scheme: {}", scheme));
            }
            items.push(format!(
                "Partitions: EFI {} | BOOT {} | ROOT {} | DATA remainder",
                app.efi_size, app.boot_size, app.root_end
            ));
            let download_efi = matches!(
                app.uefi_sources.get(app.uefi_source_index),
                Some(super::flash_config::EfiSource::DownloadEfiImage)
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
            items.push("  Enter — Submit after typing DESTROY".to_string());
            items.push("  Esc — Cancel and go back".to_string());
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
            if app.completion_lines.is_empty() {
                items.push("🎉 Installation complete.".to_string());
            } else {
                items.extend(app.completion_lines.clone());
            }
            items.push("".to_string());
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
                | InstallStepType::ExecuteConfirmationGate
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

fn build_plan_lines(app: &super::dojo_app::App) -> Vec<String> {
    let mut lines = Vec::new();

    let distro = app.os_distros.get(app.os_distro_index).copied();
    if let Some(distro) = distro {
        lines.push(format!("OS: {}", distro.display()));
    } else {
        lines.push("OS: <not selected>".to_string());
    }

    if let Some(variant) = app.os_variants.get(app.os_variant_index) {
        lines.push(format!("Variant: {}", variant.label));
    } else {
        lines.push("Variant: <not selected>".to_string());
    }

    if let Some(disk) = app.disks.get(app.disk_index) {
        let disk_info = match &disk.identity {
            Some(identity) => identity.display_string(),
            None => "❌ IDENTITY FAILED".to_string(),
        };
        lines.push(format!("Disk: {} - {}", disk.path, disk_info));
    } else {
        lines.push("Disk: <not selected>".to_string());
    }

    if matches!(distro, Some(super::flash_config::OsDistro::Fedora)) {
        if let Some(uefi_source) = app.uefi_sources.get(app.uefi_source_index) {
            match uefi_source {
                super::flash_config::EfiSource::LocalEfiImage => {
                    lines.push(format!("EFI: Local ({})", app.uefi_source_path));
                }
                super::flash_config::EfiSource::DownloadEfiImage => {
                    lines.push("EFI: Download image".to_string());
                }
            }
        } else {
            lines.push("EFI: <not selected>".to_string());
        }
        lines.push("Action: install Fedora with custom partition layout".to_string());
        lines.push("Reboots: 1".to_string());
    } else {
        lines.push("Action: flash upstream full-disk image (no repartition)".to_string());
        if matches!(distro, Some(super::flash_config::OsDistro::Manjaro)) {
            lines.push("Note: post-boot partition expansion required".to_string());
        }
        lines.push("Reboots: 0".to_string());
    }

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
        InstallStepType::PartitionLayout => {
            "Up/Down/Tab, Enter/Y (accept), M (manual), Esc, q".to_string()
        }
        InstallStepType::PartitionScheme => "Up/Down/Tab, Enter, Esc, q".to_string(),
        InstallStepType::PartitionCustomize => {
            "Up/Down/Tab, Type, Backspace, R, Enter, Esc, q".to_string()
        }
        InstallStepType::PlanReview => "Enter, Esc, q".to_string(),
        InstallStepType::Confirmation => "Enter, Esc, q".to_string(),
        InstallStepType::ExecuteConfirmationGate => "Type EXACT phrase, Enter, Esc, q".to_string(),
        InstallStepType::DisarmSafeMode => "Type DESTROY, Enter, Esc, q".to_string(),
        InstallStepType::DiskSelection
        | InstallStepType::ImageSelection
        | InstallStepType::VariantSelection
        | InstallStepType::LocaleSelection
        | InstallStepType::FirstBootUser => "Up/Down/Tab, Enter, Esc, q".to_string(),
        InstallStepType::DownloadSourceSelection | InstallStepType::EfiImage => {
            "Up/Down/Tab, Type, Backspace, Enter, Esc, q".to_string()
        }
    }
}

fn format_disk_entry(disk: &super::dojo_app::DiskOption) -> String {
    use super::data_sources::BootConfidence;

    // CRITICAL: Use DiskIdentity::display_string() exclusively - no UI-side reconstruction
    let identity_str = match &disk.identity {
        Some(identity) => identity.display_string(),
        None => {
            // Identity resolution failed - show error (should be blocked from selection)
            return format!("{} - ❌ IDENTITY FAILED - Cannot proceed", disk.path);
        }
    };

    // Build tags
    let mut tags = Vec::new();

    if disk.is_source_disk {
        tags.push("⚠ SOURCE MEDIA");
    }

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

    format!("{} - {}{}", disk.path, identity_str, tag_str)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plan_review_renders_summary() {
        let mut app = crate::dojo::dojo_app::App::new_with_flags(true);
        app.current_step_type = crate::dojo::dojo_app::InstallStepType::PlanReview;
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
