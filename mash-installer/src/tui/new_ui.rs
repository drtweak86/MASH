//! New UI module for the single-screen TUI
//!
//! This module renders the unified single-page view with:
//! - Left sidebar: Configuration and Execution step lists
//! - Right panel: Interactive content for current step
//! - Bottom bar: Progress gauge, status, warnings

use super::new_app::{App, ConfigStep, ExecutionStep, InstallMode, StepState, ImageSource};
use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Gauge, List, ListItem, Paragraph, Wrap},
    Frame,
};

/// Main draw function for the single-page UI
pub fn draw(f: &mut Frame, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints([
            Constraint::Length(3),  // Title bar
            Constraint::Min(0),     // Main content (sidebar + panel)
            Constraint::Length(3),  // Progress bar
            Constraint::Length(3),  // Status line
        ])
        .split(f.area());

    // Title bar
    draw_title(f, app, chunks[0]);

    // Main content: sidebar + panel
    draw_main_content(f, app, chunks[1]);

    // Progress bar
    draw_progress_bar(f, app, chunks[2]);

    // Status line
    draw_status_line(f, app, chunks[3]);

    // Cancel confirmation dialog (modal overlay)
    if app.ui.cancel_dialog_visible {
        draw_cancel_dialog(f, app);
    }
}

fn draw_title(f: &mut Frame, app: &App, area: Rect) {
    let dry_run_indicator = if app.dry_run { " [DRY-RUN]" } else { "" };
    let mode_indicator = match app.state.mode {
        InstallMode::Welcome => " - Welcome",
        InstallMode::Configuring => " - Configuration",
        InstallMode::Executing => " - Installing",
        InstallMode::Complete => " - Complete",
    };

    let title = Paragraph::new(format!("MASH Installer{}{}", mode_indicator, dry_run_indicator))
        .style(
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Cyan)),
        );
    f.render_widget(title, area);
}

fn draw_main_content(f: &mut Frame, app: &App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Length(32), Constraint::Min(0)])
        .split(area);

    // Left sidebar with step lists
    draw_sidebar(f, app, chunks[0]);

    // Right panel with interactive content
    draw_panel(f, app, chunks[1]);
}

fn draw_sidebar(f: &mut Frame, app: &App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(area);

    // Configuration steps
    let config_items: Vec<ListItem> = ConfigStep::all()
        .iter()
        .map(|step| {
            let state = app.state.config_states.get(step).unwrap_or(&StepState::Pending);
            let (symbol, style) = step_style(*state);
            let is_current = app.state.current_config == Some(*step);

            let line = if is_current {
                Line::from(vec![
                    Span::styled(symbol, style),
                    Span::raw(" "),
                    Span::styled(step.title(), style.add_modifier(Modifier::BOLD)),
                    Span::styled(" <", Style::default().fg(Color::Yellow)),
                ])
            } else {
                Line::from(vec![
                    Span::styled(symbol, style),
                    Span::raw(" "),
                    Span::styled(step.title(), style),
                ])
            };

            ListItem::new(line)
        })
        .collect();

    let config_list = List::new(config_items).block(
        Block::default()
            .borders(Borders::ALL)
            .title(" Configuration ")
            .border_style(Style::default().fg(Color::Blue)),
    );
    f.render_widget(config_list, chunks[0]);

    // Execution steps
    let exec_items: Vec<ListItem> = ExecutionStep::all()
        .iter()
        .map(|step| {
            let state = app.state.exec_states.get(step).unwrap_or(&StepState::Pending);
            let (symbol, style) = step_style(*state);
            let is_current = app.state.current_exec == Some(*step);

            let line = if is_current {
                Line::from(vec![
                    Span::styled(symbol, style),
                    Span::raw(" "),
                    Span::styled(step.title(), style.add_modifier(Modifier::BOLD)),
                    Span::styled(" <", Style::default().fg(Color::Yellow)),
                ])
            } else {
                Line::from(vec![
                    Span::styled(symbol, style),
                    Span::raw(" "),
                    Span::styled(step.title(), style),
                ])
            };

            ListItem::new(line)
        })
        .collect();

    let exec_list = List::new(exec_items).block(
        Block::default()
            .borders(Borders::ALL)
            .title(" Execution ")
            .border_style(Style::default().fg(Color::Magenta)),
    );
    f.render_widget(exec_list, chunks[1]);
}

fn draw_panel(f: &mut Frame, app: &App, area: Rect) {
    match app.state.mode {
        InstallMode::Welcome => draw_welcome_panel(f, app, area),
        InstallMode::Configuring => draw_config_panel(f, app, area),
        InstallMode::Executing => draw_execution_panel(f, app, area),
        InstallMode::Complete => draw_complete_panel(f, app, area),
    }
}

fn draw_welcome_panel(f: &mut Frame, _app: &App, area: Rect) {
    let text = vec![
        Line::from(""),
        Line::from(Span::styled(
            "Welcome to MASH Installer",
            Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from("This wizard will guide you through installing Fedora KDE"),
        Line::from("on your Raspberry Pi 4B with UEFI boot support."),
        Line::from(""),
        Line::from(Span::styled(
            "WARNING: This process is DESTRUCTIVE!",
            Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
        )),
        Line::from("All data on the selected disk will be erased."),
        Line::from(""),
        Line::from(""),
        Line::from(Span::styled(
            "Press Enter to begin configuration...",
            Style::default().fg(Color::Yellow),
        )),
        Line::from("Press Esc or 'q' to quit"),
    ];

    let paragraph = Paragraph::new(text)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Welcome ")
                .border_style(Style::default().fg(Color::Green)),
        )
        .alignment(Alignment::Center);
    f.render_widget(paragraph, area);
}

fn draw_config_panel(f: &mut Frame, app: &App, area: Rect) {
    let step = app.current_config_step();
    match step {
        ConfigStep::DiskSelection => draw_disk_selection_panel(f, app, area),
        ConfigStep::DiskConfirmation => draw_disk_confirm_panel(f, app, area),
        ConfigStep::PartitionScheme => draw_partition_scheme_panel(f, app, area),
        ConfigStep::PartitionLayout => draw_partition_layout_panel(f, app, area),
        ConfigStep::PartitionCustomize => draw_partition_customize_panel(f, app, area),
        ConfigStep::ImageSource => draw_image_source_panel(f, app, area),
        ConfigStep::ImageSelection => draw_image_selection_panel(f, app, area),
        ConfigStep::UefiSource => draw_uefi_source_panel(f, app, area),
        ConfigStep::LocaleSelection => draw_locale_selection_panel(f, app, area),
        ConfigStep::Options => draw_options_panel(f, app, area),
        ConfigStep::FinalSummary => draw_final_summary_panel(f, app, area),
    }
}

fn draw_execution_panel(f: &mut Frame, app: &App, area: Rect) {
    let mut lines: Vec<Line> = vec![
        Line::from(Span::styled(
            "Installation in progress...",
            Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
    ];

    // Show recent log lines
    let log_start = app.ui.exec_log.len().saturating_sub(20);
    for log_line in app.ui.exec_log.iter().skip(log_start) {
        lines.push(Line::from(Span::styled(
            format!("  {}", log_line),
            Style::default().fg(Color::Gray),
        )));
    }

    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "Press Esc to cancel (will prompt for confirmation)",
        Style::default().fg(Color::DarkGray),
    )));

    let paragraph = Paragraph::new(lines)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Installation Log ")
                .border_style(Style::default().fg(Color::Yellow)),
        )
        .wrap(Wrap { trim: true });
    f.render_widget(paragraph, area);
}

fn draw_complete_panel(f: &mut Frame, app: &App, area: Rect) {
    let mut lines = vec![
        Line::from(""),
        Line::from(Span::styled(
            if app.state.error.is_some() {
                "Installation Failed"
            } else {
                "Installation Complete!"
            },
            Style::default()
                .fg(if app.state.error.is_some() { Color::Red } else { Color::Green })
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
    ];

    if let Some(ref error) = app.state.error {
        lines.push(Line::from(Span::styled(
            format!("Error: {}", error),
            Style::default().fg(Color::Red),
        )));
        lines.push(Line::from(""));
    }

    if !app.state.cleanup_warnings.is_empty() {
        lines.push(Line::from(Span::styled(
            "Cleanup Warnings:",
            Style::default().fg(Color::Yellow),
        )));
        for warning in &app.state.cleanup_warnings {
            lines.push(Line::from(Span::styled(
                format!("  - {}", warning),
                Style::default().fg(Color::Yellow),
            )));
        }
        lines.push(Line::from(""));
    }

    if app.state.error.is_none() {
        lines.push(Line::from("Your Raspberry Pi 4B is ready!"));
        lines.push(Line::from(""));
        lines.push(Line::from("Next steps:"));
        lines.push(Line::from("  1. Remove the SD card safely"));
        lines.push(Line::from("  2. Insert into your Raspberry Pi 4B"));
        lines.push(Line::from("  3. Power on and enjoy Fedora KDE!"));
    }

    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "Press Enter or Esc to exit",
        Style::default().fg(Color::DarkGray),
    )));

    let paragraph = Paragraph::new(lines)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Complete ")
                .border_style(Style::default().fg(
                    if app.state.error.is_some() { Color::Red } else { Color::Green }
                )),
        )
        .alignment(Alignment::Center);
    f.render_widget(paragraph, area);
}

// ============================================================================
// Config Step Panels
// ============================================================================

fn draw_disk_selection_panel(f: &mut Frame, app: &App, area: Rect) {
    let mut lines = vec![
        Line::from(Span::styled(
            "Select Target Disk",
            Style::default().add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from("Use Up/Down or j/k to navigate, Enter to select"),
        Line::from("Press 'r' to refresh disk list"),
        Line::from(""),
    ];

    if app.ui.disks.is_empty() {
        lines.push(Line::from(Span::styled(
            "No removable disks found!",
            Style::default().fg(Color::Red),
        )));
        lines.push(Line::from("Insert an SD card and press 'r' to refresh."));
    } else {
        for (i, disk) in app.ui.disks.iter().enumerate() {
            let selected = i == app.ui.selected_disk_idx;
            let prefix = if selected { "> " } else { "  " };
            let style = if selected {
                Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };

            lines.push(Line::from(Span::styled(
                format!("{}{} - {} ({} GB)", prefix, disk.path, disk.model, disk.size_gb),
                style,
            )));
        }
    }

    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "Esc: Exit",
        Style::default().fg(Color::DarkGray),
    )));

    let paragraph = Paragraph::new(lines)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Disk Selection ")
                .border_style(Style::default().fg(Color::Blue)),
        );
    f.render_widget(paragraph, area);
}

fn draw_disk_confirm_panel(f: &mut Frame, app: &App, area: Rect) {
    let disk_name = app.selected_disk.as_deref().unwrap_or("(none)");

    let mut lines = vec![
        Line::from(Span::styled(
            "CONFIRM DISK DESTRUCTION",
            Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(format!("Target disk: {}", disk_name)),
        Line::from(""),
        Line::from(Span::styled(
            "ALL DATA ON THIS DISK WILL BE PERMANENTLY ERASED!",
            Style::default().fg(Color::Red),
        )),
        Line::from(""),
        Line::from("Type DESTROY to confirm:"),
        Line::from(""),
        Line::from(Span::styled(
            format!("> {}_", app.ui.disk_confirm_input),
            Style::default().fg(Color::Yellow),
        )),
    ];

    if let Some(ref error) = app.ui.disk_confirm_error {
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            error.clone(),
            Style::default().fg(Color::Red),
        )));
    }

    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "Esc: Go back",
        Style::default().fg(Color::DarkGray),
    )));

    let paragraph = Paragraph::new(lines)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Confirmation ")
                .border_style(Style::default().fg(Color::Red)),
        );
    f.render_widget(paragraph, area);
}

fn draw_partition_scheme_panel(f: &mut Frame, app: &App, area: Rect) {
    let mut lines = vec![
        Line::from(Span::styled(
            "Select Partition Scheme",
            Style::default().add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from("Choose the partition table type for your disk:"),
        Line::from(""),
    ];

    let schemes = [
        ("MBR (Master Boot Record)", "Traditional, widely compatible (RECOMMENDED)"),
        ("GPT (GUID Partition Table)", "Modern, supports larger disks"),
    ];

    for (i, (name, desc)) in schemes.iter().enumerate() {
        let selected = i == app.ui.partition_scheme_idx;
        let radio = if selected { "(o)" } else { "( )" };
        let style = if selected {
            Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
        } else {
            Style::default()
        };

        lines.push(Line::from(Span::styled(
            format!("{} {}", radio, name),
            style,
        )));
        lines.push(Line::from(Span::styled(
            format!("    {}", desc),
            Style::default().fg(Color::DarkGray),
        )));
    }

    lines.push(Line::from(""));
    lines.push(Line::from("Use Up/Down to select, Enter to confirm"));
    lines.push(Line::from(Span::styled(
        "Esc: Go back",
        Style::default().fg(Color::DarkGray),
    )));

    let paragraph = Paragraph::new(lines)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Partition Scheme ")
                .border_style(Style::default().fg(Color::Blue)),
        );
    f.render_widget(paragraph, area);
}

fn draw_partition_layout_panel(f: &mut Frame, app: &App, area: Rect) {
    let lines = vec![
        Line::from(Span::styled(
            "Partition Layout",
            Style::default().add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from("Recommended partition sizes:"),
        Line::from(""),
        Line::from(format!("  EFI:  {} (FAT32, bootloader)", app.partition_plan.efi_size.display())),
        Line::from(format!("  BOOT: {} (ext4, kernel)", app.partition_plan.boot_size.display())),
        Line::from(format!("  ROOT: up to {} (ext4, system)", app.partition_plan.root_end.display())),
        Line::from(""),
        Line::from(Span::styled(
            "Use recommended layout?",
            Style::default().fg(Color::Yellow),
        )),
        Line::from(""),
        Line::from("  [Y] Yes, use recommended (default)"),
        Line::from("  [N] No, customize sizes"),
        Line::from(""),
        Line::from(Span::styled(
            "Esc: Go back",
            Style::default().fg(Color::DarkGray),
        )),
    ];

    let paragraph = Paragraph::new(lines)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Partition Layout ")
                .border_style(Style::default().fg(Color::Blue)),
        );
    f.render_widget(paragraph, area);
}

fn draw_partition_customize_panel(f: &mut Frame, app: &App, area: Rect) {
    let fields = [
        ("EFI Size", &app.partition_plan.efi_size),
        ("Boot Size", &app.partition_plan.boot_size),
        ("Root End", &app.partition_plan.root_end),
    ];

    let mut lines = vec![
        Line::from(Span::styled(
            "Customize Partition Sizes",
            Style::default().add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from("Enter sizes with units: MiB, GiB (e.g., 1024MiB, 8GiB)"),
        Line::from("Use Tab/Down to move between fields, Enter to confirm"),
        Line::from(""),
    ];

    for (i, (label, size)) in fields.iter().enumerate() {
        let selected = i == app.ui.partition_edit_idx;
        let prefix = if selected { "> " } else { "  " };
        let style = if selected {
            Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
        } else {
            Style::default()
        };

        if selected {
            lines.push(Line::from(Span::styled(
                format!("{}{}: {}_", prefix, label, app.ui.partition_edit_input),
                style,
            )));
        } else {
            lines.push(Line::from(Span::styled(
                format!("{}{}: {}", prefix, label, size.display()),
                style,
            )));
        }
    }

    if let Some(ref error) = app.ui.partition_edit_error {
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            error.clone(),
            Style::default().fg(Color::Red),
        )));
    }

    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "Esc: Go back",
        Style::default().fg(Color::DarkGray),
    )));

    let paragraph = Paragraph::new(lines)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Partition Sizes ")
                .border_style(Style::default().fg(Color::Blue)),
        );
    f.render_widget(paragraph, area);
}

fn draw_image_source_panel(f: &mut Frame, app: &App, area: Rect) {
    let mut lines = vec![
        Line::from(Span::styled(
            "Select Image Source",
            Style::default().add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from("Choose where to get the Fedora image:"),
        Line::from(""),
    ];

    let sources = [
        ("Local File", "Use an existing .raw image file"),
        ("Download Fedora", "Download official Fedora ARM image"),
    ];

    for (i, (name, desc)) in sources.iter().enumerate() {
        let selected = i == app.ui.image_source_idx;
        let radio = if selected { "(o)" } else { "( )" };
        let style = if selected {
            Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
        } else {
            Style::default()
        };

        lines.push(Line::from(Span::styled(
            format!("{} {}", radio, name),
            style,
        )));
        lines.push(Line::from(Span::styled(
            format!("    {}", desc),
            Style::default().fg(Color::DarkGray),
        )));
    }

    lines.push(Line::from(""));
    lines.push(Line::from("Use Up/Down to select, Enter to confirm"));
    lines.push(Line::from(Span::styled(
        "Esc: Go back",
        Style::default().fg(Color::DarkGray),
    )));

    let paragraph = Paragraph::new(lines)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Image Source ")
                .border_style(Style::default().fg(Color::Blue)),
        );
    f.render_widget(paragraph, area);
}

fn draw_image_selection_panel(f: &mut Frame, app: &App, area: Rect) {
    let mut lines = vec![
        Line::from(Span::styled(
            if app.image_source == ImageSource::LocalFile {
                "Select Local Image File"
            } else {
                "Select Fedora Version"
            },
            Style::default().add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
    ];

    match app.image_source {
        ImageSource::LocalFile => {
            lines.push(Line::from("Enter the path to your .raw image file:"));
            lines.push(Line::from(""));
            lines.push(Line::from(Span::styled(
                format!("> {}_", app.ui.image_path_input),
                Style::default().fg(Color::Yellow),
            )));

            if let Some(ref error) = app.ui.image_path_error {
                lines.push(Line::from(""));
                lines.push(Line::from(Span::styled(
                    error.clone(),
                    Style::default().fg(Color::Red),
                )));
            }
        }
        ImageSource::DownloadFedora => {
            lines.push(Line::from("Select Fedora version to download:"));
            lines.push(Line::from(""));

            let versions = ["Fedora 43 (Latest)", "Fedora 42"];

            for (i, version) in versions.iter().enumerate() {
                let selected = i == app.ui.image_version_idx;
                let radio = if selected { "(o)" } else { "( )" };
                let style = if selected {
                    Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
                } else {
                    Style::default()
                };

                lines.push(Line::from(Span::styled(
                    format!("{} {}", radio, version),
                    style,
                )));
            }
        }
    }

    lines.push(Line::from(""));
    lines.push(Line::from("Press Enter to continue"));
    lines.push(Line::from(Span::styled(
        "Esc: Go back",
        Style::default().fg(Color::DarkGray),
    )));

    let paragraph = Paragraph::new(lines)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Image Selection ")
                .border_style(Style::default().fg(Color::Blue)),
        );
    f.render_widget(paragraph, area);
}

fn draw_uefi_source_panel(f: &mut Frame, app: &App, area: Rect) {
    let mut lines = vec![
        Line::from(Span::styled(
            "UEFI Firmware",
            Style::default().add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from("Raspberry Pi UEFI firmware is required for booting."),
        Line::from(""),
        Line::from(Span::styled(
            format!(
                "[{}] Download UEFI firmware automatically (Press 'd' to toggle)",
                if app.ui.uefi_download { "X" } else { " " }
            ),
            if app.ui.uefi_download {
                Style::default().fg(Color::Green)
            } else {
                Style::default()
            },
        )),
        Line::from(""),
    ];

    if !app.ui.uefi_download {
        lines.push(Line::from("Enter path to existing UEFI directory:"));
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            format!("> {}_", app.ui.uefi_path_input),
            Style::default().fg(Color::Yellow),
        )));

        if let Some(ref error) = app.ui.uefi_path_error {
            lines.push(Line::from(""));
            lines.push(Line::from(Span::styled(
                error.clone(),
                Style::default().fg(Color::Red),
            )));
        }
    } else {
        lines.push(Line::from("UEFI firmware will be downloaded from GitHub."));
    }

    lines.push(Line::from(""));
    lines.push(Line::from("Press Enter or Tab to continue"));
    lines.push(Line::from(Span::styled(
        "Esc: Go back",
        Style::default().fg(Color::DarkGray),
    )));

    let paragraph = Paragraph::new(lines)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" UEFI Firmware ")
                .border_style(Style::default().fg(Color::Blue)),
        );
    f.render_widget(paragraph, area);
}

fn draw_locale_selection_panel(f: &mut Frame, app: &App, area: Rect) {
    let mut lines = vec![
        Line::from(Span::styled(
            "Select Locale",
            Style::default().add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from("Choose your language and keyboard layout:"),
        Line::from(""),
    ];

    // Show a subset of locales around the selected one
    let total = app.ui.locales.len();
    let start = app.ui.selected_locale_idx.saturating_sub(5);
    let end = (start + 11).min(total);

    for i in start..end {
        let locale = &app.ui.locales[i];
        let selected = i == app.ui.selected_locale_idx;
        let prefix = if selected { "> " } else { "  " };
        let style = if selected {
            Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
        } else {
            Style::default()
        };

        lines.push(Line::from(Span::styled(
            format!("{}{} - {} ({})", prefix, locale.code, locale.name, locale.keymap),
            style,
        )));
    }

    if end < total {
        lines.push(Line::from(Span::styled(
            format!("  ... and {} more", total - end),
            Style::default().fg(Color::DarkGray),
        )));
    }

    lines.push(Line::from(""));
    lines.push(Line::from("Use Up/Down or j/k to navigate, Enter to select"));
    lines.push(Line::from(Span::styled(
        "Esc: Go back",
        Style::default().fg(Color::DarkGray),
    )));

    let paragraph = Paragraph::new(lines)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Locale ")
                .border_style(Style::default().fg(Color::Blue)),
        );
    f.render_widget(paragraph, area);
}

fn draw_options_panel(f: &mut Frame, app: &App, area: Rect) {
    let options = [
        ("Auto-unmount", app.auto_unmount, "Automatically unmount disk partitions before flashing"),
        ("Early SSH", app.early_ssh, "Enable SSH access on first boot"),
        (
            match app.partition_plan.scheme {
                crate::cli::PartitionScheme::Mbr => "Scheme: MBR",
                crate::cli::PartitionScheme::Gpt => "Scheme: GPT",
            },
            true, // Always shown as current value
            "Partition table type (set earlier)",
        ),
        ("Dry Run", app.dry_run, "Test without writing to disk"),
    ];

    let mut lines = vec![
        Line::from(Span::styled(
            "Additional Options",
            Style::default().add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from("Use Up/Down to navigate, Space/Enter to toggle"),
        Line::from("Press Tab to continue"),
        Line::from(""),
    ];

    for (i, (label, value, desc)) in options.iter().enumerate() {
        let focused = i == app.ui.options_focus_idx;
        let checkbox = if i == 2 {
            // Partition scheme - not a toggle
            format!("  {}", label)
        } else if *value {
            format!("[X] {}", label)
        } else {
            format!("[ ] {}", label)
        };

        let style = if focused {
            Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
        } else {
            Style::default()
        };

        lines.push(Line::from(Span::styled(checkbox, style)));
        lines.push(Line::from(Span::styled(
            format!("    {}", desc),
            Style::default().fg(Color::DarkGray),
        )));
    }

    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "Esc: Go back",
        Style::default().fg(Color::DarkGray),
    )));

    let paragraph = Paragraph::new(lines)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Options ")
                .border_style(Style::default().fg(Color::Blue)),
        );
    f.render_widget(paragraph, area);
}

fn draw_final_summary_panel(f: &mut Frame, app: &App, area: Rect) {
    let disk = app.selected_disk.as_deref().unwrap_or("(not selected)");
    let image = app.image_path
        .as_ref()
        .map(|p| p.display().to_string())
        .unwrap_or_else(|| {
            if app.image_source == ImageSource::DownloadFedora {
                "Will be downloaded".to_string()
            } else {
                "(not selected)".to_string()
            }
        });
    let uefi = if app.download_uefi {
        "Will be downloaded".to_string()
    } else {
        app.uefi_dir
            .as_ref()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|| "(not selected)".to_string())
    };
    let locale = app.locale
        .as_ref()
        .map(|l| format!("{} ({})", l.name, l.code))
        .unwrap_or_else(|| "(default)".to_string());

    let mut lines = vec![
        Line::from(Span::styled(
            "Final Summary - Ready to Flash",
            Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(format!("Target Disk:  {}", disk)),
        Line::from(format!("Image:        {}", image)),
        Line::from(format!("UEFI:         {}", uefi)),
        Line::from(format!("Locale:       {}", locale)),
        Line::from(format!("Scheme:       {:?}", app.partition_plan.scheme)),
        Line::from(format!("Auto-unmount: {}", if app.auto_unmount { "Yes" } else { "No" })),
        Line::from(format!("Early SSH:    {}", if app.early_ssh { "Yes" } else { "No" })),
        Line::from(format!("Dry Run:      {}", if app.dry_run { "Yes" } else { "No" })),
        Line::from(""),
        Line::from(Span::styled(
            "THIS IS YOUR FINAL CHANCE TO ABORT!",
            Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from("Type FLASH to begin installation:"),
        Line::from(""),
        Line::from(Span::styled(
            format!("> {}_", app.ui.flash_confirm_input),
            Style::default().fg(Color::Yellow),
        )),
    ];

    if let Some(ref error) = app.ui.flash_confirm_error {
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            error.clone(),
            Style::default().fg(Color::Red),
        )));
    }

    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "Esc: Go back and review settings",
        Style::default().fg(Color::DarkGray),
    )));

    let paragraph = Paragraph::new(lines)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Summary ")
                .border_style(Style::default().fg(Color::Red)),
        );
    f.render_widget(paragraph, area);
}

// ============================================================================
// Progress and Status
// ============================================================================

fn draw_progress_bar(f: &mut Frame, app: &App, area: Rect) {
    let percent = app.state.overall_percent.min(100.0) as u16;

    let gauge_style = if app.state.error.is_some() {
        Style::default().fg(Color::Red)
    } else if percent >= 100 {
        Style::default().fg(Color::Green)
    } else {
        Style::default().fg(Color::Yellow)
    };

    let gauge = Gauge::default()
        .block(Block::default().borders(Borders::ALL).title(" Progress "))
        .gauge_style(gauge_style)
        .percent(percent)
        .label(format!("{}%", percent));
    f.render_widget(gauge, area);
}

fn draw_status_line(f: &mut Frame, app: &App, area: Rect) {
    let (message, style) = if let Some(ref error) = app.state.error {
        (format!("Error: {}", error), Style::default().fg(Color::Red))
    } else if !app.state.cleanup_warnings.is_empty() {
        (
            format!(
                "{} ({} cleanup warnings)",
                app.state.status_message,
                app.state.cleanup_warnings.len()
            ),
            Style::default().fg(Color::Yellow),
        )
    } else {
        (app.state.status_message.clone(), Style::default().fg(Color::Gray))
    };

    let status = Paragraph::new(message)
        .style(style)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Status ")
                .border_style(Style::default().fg(Color::DarkGray)),
        );
    f.render_widget(status, area);
}

// ============================================================================
// Cancel Dialog (Modal)
// ============================================================================

fn draw_cancel_dialog(f: &mut Frame, _app: &App) {
    let area = centered_rect(50, 30, f.area());

    // Clear the area first
    f.render_widget(Clear, area);

    let lines = vec![
        Line::from(""),
        Line::from(Span::styled(
            "Cancel Installation?",
            Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from("Installation is in progress."),
        Line::from("Cancelling will run cleanup to safely:"),
        Line::from("  - Unmount all mounted partitions"),
        Line::from("  - Detach loop devices"),
        Line::from("  - Remove temporary files"),
        Line::from(""),
        Line::from(Span::styled(
            "Are you sure you want to cancel?",
            Style::default().fg(Color::Red),
        )),
        Line::from(""),
        Line::from(Span::styled(
            "[Y] Yes, cancel and cleanup",
            Style::default().fg(Color::Red),
        )),
        Line::from(Span::styled(
            "[N] No, continue installation",
            Style::default().fg(Color::Green),
        )),
    ];

    let paragraph = Paragraph::new(lines)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Confirm Cancel ")
                .border_style(Style::default().fg(Color::Red)),
        )
        .alignment(Alignment::Center);
    f.render_widget(paragraph, area);
}

/// Helper function to create a centered rect
fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}

// ============================================================================
// Helpers
// ============================================================================

/// Get the display symbol and style for a step state
fn step_style(state: StepState) -> (&'static str, Style) {
    match state {
        StepState::Pending => ("[ ]", Style::default().fg(Color::DarkGray)),
        StepState::Current => ("[>]", Style::default().fg(Color::Yellow)),
        StepState::Completed => ("[+]", Style::default().fg(Color::Green)),
        StepState::Skipped => ("[-]", Style::default().fg(Color::DarkGray)),
        StepState::Failed => ("[!]", Style::default().fg(Color::Red)),
    }
}
