//! UI rendering for the TUI

use super::app::{App, Screen};
use super::input::InputMode;
use super::progress::Phase;
use super::widgets::CheckboxState;
use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Gauge, List, ListItem, Paragraph, Wrap},
    Frame,
};

/// Main draw function
pub fn draw(f: &mut Frame, app: &App) {
    // Create main layout
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Title bar
            Constraint::Min(10),   // Main content
            Constraint::Length(3), // Help bar
        ])
        .split(f.area());

    // Draw title bar
    draw_title_bar(f, app, chunks[0]);

    // Draw main content based on current screen
    match app.current_screen {
        Screen::Welcome => draw_welcome(f, app, chunks[1]),
        Screen::DiskSelection => draw_disk_selection(f, app, chunks[1]),
        Screen::ImageSelection => draw_image_selection(f, app, chunks[1]),
        Screen::UefiDirectory => draw_uefi_selection(f, app, chunks[1]),
        Screen::LocaleSelection => draw_locale_selection(f, app, chunks[1]),
        Screen::Options => draw_options(f, app, chunks[1]),
        Screen::Confirmation => draw_confirmation(f, app, chunks[1]),
        Screen::Progress => draw_progress(f, app, chunks[1]),
        Screen::Complete => draw_complete(f, app, chunks[1]),
    }

    // Draw help bar
    draw_help_bar(f, app, chunks[2]);
}

fn draw_title_bar(f: &mut Frame, app: &App, area: Rect) {
    let title = format!(
        " 🍠 MASH - {} ",
        app.current_screen.title()
    );

    let mode_indicator = if app.options.dry_run || app.dry_run_cli {
        " [🧪 DRY-RUN] "
    } else {
        ""
    };

    let title_block = Paragraph::new(format!("{}{}", title, mode_indicator))
        .style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD))
        .alignment(Alignment::Center)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Cyan)),
        );

    f.render_widget(title_block, area);
}

fn draw_help_bar(f: &mut Frame, app: &App, area: Rect) {
    let help_text = match app.current_screen {
        Screen::Welcome => "⏎ Enter: Start | Esc/q: Quit",
        Screen::DiskSelection => "↑↓: Select | ⏎ Enter: Confirm | r: Refresh | Esc: Back",
        Screen::ImageSelection => {
            if app.image_input.mode == InputMode::Editing {
                "⏎ Enter: Confirm | Esc: Cancel editing"
            } else {
                "⏎/e/i: Edit | Tab: Next | Esc: Back"
            }
        }
        Screen::UefiDirectory => {
            if app.uefi_input.mode == InputMode::Editing {
                "⏎ Enter: Confirm | Esc: Cancel editing"
            } else {
                "⏎/e/i: Edit | Tab: Next | Esc: Back"
            }
        }
        Screen::LocaleSelection => "↑↓: Select | ⏎/Tab: Next | Esc: Back",
        Screen::Options => "↑↓: Navigate | Space/⏎: Toggle | Tab: Next | Esc: Back",
        Screen::Confirmation => "Type 'YES I KNOW' to confirm | Esc: Back",
        Screen::Progress => "Ctrl+C: Abort",
        Screen::Complete => "⏎/Esc/q: Exit",
    };

    let help = Paragraph::new(help_text)
        .style(Style::default().fg(Color::DarkGray))
        .alignment(Alignment::Center)
        .block(Block::default().borders(Borders::ALL));

    f.render_widget(help, area);
}

fn draw_welcome(f: &mut Frame, app: &App, area: Rect) {
    // Animated cursor blink
    let cursor = if (app.animation_tick / 5) % 2 == 0 { "▌" } else { " " };

    let text = vec![
        Line::from(""),
        Line::from(""),
        Line::from(Span::styled(
            "🥋 Enter the Dojo 🥋",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(""),
        Line::from("🍠 MASH Installer will guide you through:"),
        Line::from(""),
        Line::from("  1️⃣  Select target disk (will be ERASED)"),
        Line::from("  2️⃣  Select Fedora image file"),
        Line::from("  3️⃣  Configure UEFI overlay"),
        Line::from("  4️⃣  Choose locale and keymap"),
        Line::from("  5️⃣  Set installation options"),
        Line::from("  6️⃣  Flash the image"),
        Line::from(""),
        Line::from(""),
        Line::from(vec![
            Span::styled(
                "Press ENTER to begin...",
                Style::default().fg(Color::Green),
            ),
            Span::styled(cursor, Style::default().fg(Color::Green)),
        ]),
    ];

    let paragraph = Paragraph::new(text)
        .alignment(Alignment::Center)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" 🎉 Welcome ")
                .border_style(Style::default().fg(Color::Yellow)),
        );

    f.render_widget(paragraph, area);
}

fn draw_disk_selection(f: &mut Frame, app: &App, area: Rect) {
    let items: Vec<ListItem> = app
        .available_disks
        .iter()
        .enumerate()
        .map(|(i, disk)| {
            let style = if i == app.selected_disk_index {
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };

            let prefix = if i == app.selected_disk_index {
                "👉 "
            } else {
                "   "
            };

            ListItem::new(format!("{}{}", prefix, disk.display())).style(style)
        })
        .collect();

    let list = List::new(items).block(
        Block::default()
            .borders(Borders::ALL)
            .title(" 💾 Select Target Disk (will be ERASED!) ⚠️ ")
            .border_style(Style::default().fg(Color::Red)),
    );

    if app.available_disks.is_empty() {
        let no_disks = Paragraph::new("😕 No removable disks found.\n\n🔄 Press 'r' to refresh.")
            .alignment(Alignment::Center)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(" 💾 Select Target Disk ")
                    .border_style(Style::default().fg(Color::Red)),
            );
        f.render_widget(no_disks, area);
    } else {
        f.render_widget(list, area);
    }
}

fn draw_image_selection(f: &mut Frame, app: &App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Length(3),
            Constraint::Min(3),
        ])
        .margin(1)
        .split(area);

    // Input field
    let input_style = if app.image_input.mode == InputMode::Editing {
        Style::default().fg(Color::Yellow)
    } else {
        Style::default()
    };

    let input = Paragraph::new(app.image_input.value())
        .style(input_style)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(format!(
                    " 📀 {} {} ",
                    app.image_input.placeholder,
                    if app.image_input.mode == InputMode::Editing {
                        "✏️"
                    } else {
                        ""
                    }
                ))
                .border_style(if app.image_input.mode == InputMode::Editing {
                    Style::default().fg(Color::Yellow)
                } else {
                    Style::default()
                }),
        );

    f.render_widget(input, chunks[0]);

    // Show cursor when editing
    if app.image_input.mode == InputMode::Editing {
        f.set_cursor_position((
            chunks[0].x + app.image_input.cursor() as u16 + 1,
            chunks[0].y + 1,
        ));
    }

    // Error message
    if let Some(ref err) = app.image_error {
        let error = Paragraph::new(format!("❌ {}", err))
            .style(Style::default().fg(Color::Red))
            .block(Block::default().borders(Borders::ALL).title(" ⚠️ Error "));
        f.render_widget(error, chunks[1]);
    }

    // Help text
    let help = Paragraph::new(
        "📝 Enter the full path to a Fedora .raw image file.\n\
         📦 The image will be loop-mounted and copied to the target disk.",
    )
    .wrap(Wrap { trim: true })
    .block(Block::default().borders(Borders::ALL).title(" 💡 Help "));

    f.render_widget(help, chunks[2]);

    // Outer block
    let outer = Block::default()
        .borders(Borders::ALL)
        .title(" 📀 Select Image File ");
    f.render_widget(outer, area);
}

fn draw_uefi_selection(f: &mut Frame, app: &App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Length(3),
            Constraint::Min(3),
        ])
        .margin(1)
        .split(area);

    // Input field
    let input_style = if app.uefi_input.mode == InputMode::Editing {
        Style::default().fg(Color::Yellow)
    } else {
        Style::default()
    };

    let input = Paragraph::new(app.uefi_input.value())
        .style(input_style)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(format!(
                    " 🔧 {} {} ",
                    app.uefi_input.placeholder,
                    if app.uefi_input.mode == InputMode::Editing {
                        "✏️"
                    } else {
                        ""
                    }
                ))
                .border_style(if app.uefi_input.mode == InputMode::Editing {
                    Style::default().fg(Color::Yellow)
                } else {
                    Style::default()
                }),
        );

    f.render_widget(input, chunks[0]);

    // Show cursor when editing
    if app.uefi_input.mode == InputMode::Editing {
        f.set_cursor_position((
            chunks[0].x + app.uefi_input.cursor() as u16 + 1,
            chunks[0].y + 1,
        ));
    }

    // Error message
    if let Some(ref err) = app.uefi_error {
        let error = Paragraph::new(format!("❌ {}", err))
            .style(Style::default().fg(Color::Red))
            .block(Block::default().borders(Borders::ALL).title(" ⚠️ Error "));
        f.render_widget(error, chunks[1]);
    }

    // Help text
    let help = Paragraph::new(
        "📁 Directory containing UEFI files for Raspberry Pi 4.\n\
         🎩 These will be copied onto the EFI partition.",
    )
    .wrap(Wrap { trim: true })
    .block(Block::default().borders(Borders::ALL).title(" 💡 Help "));

    f.render_widget(help, chunks[2]);

    // Outer block
    let outer = Block::default()
        .borders(Borders::ALL)
        .title(" 🔧 UEFI Configuration ");
    f.render_widget(outer, area);
}

fn draw_locale_selection(f: &mut Frame, app: &App, area: Rect) {
    let items: Vec<ListItem> = app
        .available_locales
        .iter()
        .enumerate()
        .map(|(i, locale)| {
            let style = if i == app.selected_locale_index {
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };

            let prefix = if i == app.selected_locale_index {
                "👉 "
            } else {
                "   "
            };

            // Add flag emoji based on locale
            let flag = match locale.lang {
                "en_GB.UTF-8" => "🇬🇧",
                "en_US.UTF-8" => "🇺🇸",
                "de_DE.UTF-8" => "🇩🇪",
                "fr_FR.UTF-8" => "🇫🇷",
                "es_ES.UTF-8" => "🇪🇸",
                _ => "🌍",
            };

            ListItem::new(format!(
                "{}{} {} (⌨️ {})",
                prefix, flag, locale.lang, locale.keymap
            ))
            .style(style)
        })
        .collect();

    let list = List::new(items).block(
        Block::default()
            .borders(Borders::ALL)
            .title(" 🌍 Select Locale & Keymap ")
            .border_style(Style::default().fg(Color::Cyan)),
    );

    f.render_widget(list, area);
}

fn draw_options(f: &mut Frame, app: &App, area: Rect) {
    // Options are rendered as a focusable list. Space/Enter toggles the focused row.
    // For partition scheme we "toggle" between MBR <-> GPT.
    let scheme_label = format!(
        "🧭 Partition scheme: {}{}",
        app.options.partition_scheme,
        if matches!(app.options.partition_scheme, crate::cli::PartitionScheme::Mbr) {
            " (recommended)"
        } else {
            ""
        }
    );

    let options = [
        (
            "🔌 Auto-unmount target disk mounts".to_string(),
            Some(app.options.auto_unmount),
            "Automatically unmount any partitions from the target disk".to_string(),
        ),
        (
            "🔐 Enable Early SSH".to_string(),
            Some(app.options.early_ssh),
            "Enable SSH access before graphical login (recommended)".to_string(),
        ),
        (
            scheme_label,
            None,
            "Toggle between MBR (msdos) and GPT partition tables".to_string(),
        ),
        (
            "🧪 Dry-run mode".to_string(),
            Some(app.options.dry_run),
            "Print what would happen without making changes".to_string(),
        ),
    ];

    let items: Vec<ListItem> = options
        .iter()
        .enumerate()
        .map(|(i, (label, checked_opt, desc))| {
            let style = if i == app.options_focus {
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };

            let prefix = if i == app.options_focus { "👉 " } else { "   " };

            let symbol = match checked_opt {
                Some(checked) => CheckboxState::from(*checked).symbol().to_string(),
                None => "🔁".to_string(),
            };

            let text = format!("{}{} {}
      📝 {}", prefix, symbol, label, desc);
            ListItem::new(text).style(style)
        })
        .collect();

    let list = List::new(items).block(
        Block::default()
            .borders(Borders::ALL)
            .title(" ⚙️ Installation Options ")
            .border_style(Style::default().fg(Color::Cyan)),
    );

    f.render_widget(list, area);
}

fn draw_confirmation(f: &mut Frame, app: &App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(10),  // Summary
            Constraint::Length(5), // Input
            Constraint::Length(3), // Error
        ])
        .margin(1)
        .split(area);

    // Summary
    let disk_display = app
        .selected_disk()
        .map(|d| d.display())
        .unwrap_or_else(|| "None selected".to_string());
    let locale_display = app
        .selected_locale()
        .map(|l| format!("{} ({})", l.lang, l.keymap))
        .unwrap_or_else(|| "None selected".to_string());

    let summary_text = vec![
        Line::from(Span::styled(
            "⚠️  DANGER ZONE ⚠️",
            Style::default()
                .fg(Color::Red)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(format!("💾 Target Disk: {}", disk_display)),
        Line::from(format!("📀 Image: {}", app.image_input.value())),
        Line::from(format!("🔧 UEFI Dir: {}", app.uefi_input.value())),
        Line::from(format!("🌍 Locale: {}", locale_display)),
        Line::from(format!("🧭 Partition Scheme: {}", app.options.partition_scheme)),
        Line::from(format!(
            "🔐 Early SSH: {}",
            if app.options.early_ssh { "✅ Yes" } else { "❌ No" }
        )),
        Line::from(format!(
            "🧪 Dry-run: {}",
            if app.options.dry_run || app.dry_run_cli {
                "✅ Yes"
            } else {
                "❌ No"
            }
        )),
        Line::from(""),
        Line::from(Span::styled(
            "🔥 This will ERASE the target disk! 🔥",
            Style::default().fg(Color::Red),
        )),
    ];

    let summary = Paragraph::new(summary_text)
        .alignment(Alignment::Center)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" 📋 Installation Summary ")
                .border_style(Style::default().fg(Color::Red)),
        );

    f.render_widget(summary, chunks[0]);

    // Confirmation input
    let input_text = format!(
        "🔒 Type 'YES I KNOW' to confirm: {}",
        app.confirmation_input
    );
    let input = Paragraph::new(input_text)
        .style(Style::default().fg(Color::Yellow))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" ✍️ Confirmation ")
                .border_style(Style::default().fg(Color::Yellow)),
        );

    f.render_widget(input, chunks[1]);

    // Set cursor position
    f.set_cursor_position((
        chunks[1].x + 38 + app.confirmation_input.len() as u16,
        chunks[1].y + 1,
    ));

    // Error
    if let Some(ref err) = app.confirmation_error {
        let error = Paragraph::new(format!("❌ {}", err))
            .style(Style::default().fg(Color::Red))
            .alignment(Alignment::Center);
        f.render_widget(error, chunks[2]);
    }

    // Outer block
    let outer = Block::default()
        .borders(Borders::ALL)
        .title(" ⚠️ Confirm Installation ⚠️ ")
        .border_style(Style::default().fg(Color::Red));
    f.render_widget(outer, area);
}

fn draw_progress(f: &mut Frame, app: &App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),  // Overall progress bar
            Constraint::Min(12),    // Analytics + Phases (split horizontally)
            Constraint::Length(3),  // Status
        ])
        .margin(1)
        .split(area);

    // Overall progress bar with animated fill
    let progress_label = format!(
        "{}%  ⏱️ {}  🎯 ETA: {}",
        app.progress.overall_percent as u32,
        app.progress.elapsed_string(),
        app.progress.eta_string()
    );

    let gauge = Gauge::default()
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" 📊 Overall Progress "),
        )
        .gauge_style(Style::default().fg(Color::Green))
        .percent(app.progress.overall_percent as u16)
        .label(progress_label);

    f.render_widget(gauge, chunks[0]);

    // Split middle area into Analytics (left) and Phases (right)
    let middle_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(45), // Analytics
            Constraint::Percentage(55), // Phases
        ])
        .split(chunks[1]);

    // Analytics panel
    draw_analytics_panel(f, app, middle_chunks[0]);

    // Phase list
    draw_phase_list(f, app, middle_chunks[1]);

    // Status message with animated spinner
    let status_style = if app.progress.error.is_some() {
        Style::default().fg(Color::Red)
    } else {
        Style::default().fg(Color::Cyan)
    };

    let spinner = if let Some(phase) = app.progress.current_phase {
        phase.spinner_frame(app.animation_tick)
    } else {
        "🚀"
    };

    let status_text = format!("{} {}", spinner, app.progress.status);
    let status = Paragraph::new(status_text)
        .style(status_style)
        .alignment(Alignment::Center)
        .block(Block::default().borders(Borders::ALL).title(" 📝 Status "));

    f.render_widget(status, chunks[2]);

    // Outer block
    let outer = Block::default()
        .borders(Borders::ALL)
        .title(format!(
            " 🔥 Installing - Phase {}/{} ",
            app.progress
                .current_phase
                .map(|p| p.number())
                .unwrap_or(0),
            Phase::total()
        ))
        .border_style(Style::default().fg(Color::Yellow));
    f.render_widget(outer, area);
}

fn draw_analytics_panel(f: &mut Frame, app: &App, area: Rect) {
    let analytics_lines = vec![
        Line::from(Span::styled(
            "📊 Analytics",
            Style::default().add_modifier(Modifier::BOLD),
        )),
        Line::from("─────────────────────"),
        Line::from(format!(
            "⚡ Speed:     {:.1} MB/s",
            app.progress.rsync_speed
        )),
        Line::from(format!(
            "📈 Average:   {:.1} MB/s",
            app.progress.average_speed
        )),
        Line::from(format!(
            "🚀 Peak:      {:.1} MB/s",
            app.progress.peak_speed
        )),
        Line::from(""),
        Line::from(Span::styled(
            "⏱️ Time",
            Style::default().add_modifier(Modifier::BOLD),
        )),
        Line::from("─────────────────────"),
        Line::from(format!(
            "⏳ Elapsed:   {}",
            app.progress.elapsed_string()
        )),
        Line::from(format!("🎯 ETA:       {}", app.progress.eta_string())),
        Line::from(format!(
            "📍 Phase:     {}",
            app.progress.phase_elapsed_string()
        )),
        Line::from(""),
        Line::from(Span::styled(
            "📁 Files",
            Style::default().add_modifier(Modifier::BOLD),
        )),
        Line::from("─────────────────────"),
        Line::from(format!(
            "📄 Copied:    {} / {}",
            app.progress.files_done, app.progress.files_total
        )),
    ];

    let analytics = Paragraph::new(analytics_lines)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" 📊 Analytics ")
                .border_style(Style::default().fg(Color::Cyan)),
        );

    f.render_widget(analytics, area);
}

fn draw_phase_list(f: &mut Frame, app: &App, area: Rect) {
    let phase_items: Vec<ListItem> = Phase::all()
        .iter()
        .map(|phase| {
            let symbol = if app.progress.completed_phases.contains(phase) {
                "✅"
            } else if app.progress.current_phase == Some(*phase) {
                phase.spinner_frame(app.animation_tick)
            } else {
                "⏸️"
            };

            let style = if app.progress.current_phase == Some(*phase) {
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD)
            } else if app.progress.completed_phases.contains(phase) {
                Style::default().fg(Color::Green)
            } else {
                Style::default().fg(Color::DarkGray)
            };

            ListItem::new(format!("  {} {}", symbol, phase.name())).style(style)
        })
        .collect();

    let phase_list = List::new(phase_items).block(
        Block::default()
            .borders(Borders::ALL)
            .title(" 📋 Phases ")
            .border_style(Style::default().fg(Color::Magenta)),
    );

    f.render_widget(phase_list, area);
}

fn draw_complete(f: &mut Frame, app: &App, area: Rect) {
    // Celebration animation for success
    let sparkle = if app.install_success {
        match (app.animation_tick / 3) % 4 {
            0 => "✨",
            1 => "🎉",
            2 => "🎊",
            _ => "⭐",
        }
    } else {
        "💔"
    };

    let (title, text, style) = if app.install_success {
        (
            format!(" {} Installation Complete! {} ", sparkle, sparkle),
            vec![
                Line::from(""),
                Line::from(Span::styled(
                    "🎉 Installation completed successfully! 🎉",
                    Style::default()
                        .fg(Color::Green)
                        .add_modifier(Modifier::BOLD),
                )),
                Line::from(""),
                Line::from("📋 Next steps:"),
                Line::from(""),
                Line::from("  1️⃣  Remove the disk from this computer"),
                Line::from("  2️⃣  Insert into your Raspberry Pi 4"),
                Line::from("  3️⃣  Boot with UEFI"),
                Line::from("  4️⃣  Run Dojo setup: sudo /data/mash-staging/install_dojo.sh"),
                Line::from(""),
                Line::from(""),
                Line::from(Span::styled(
                    "🍠 Press Enter to exit - Enjoy your MASH! 🍠",
                    Style::default().fg(Color::Cyan),
                )),
            ],
            Style::default().fg(Color::Green),
        )
    } else {
        let error_msg = app
            .install_error
            .as_ref()
            .map(|e| e.as_str())
            .unwrap_or("Unknown error");
        (
            " ❌ Installation Failed ".to_string(),
            vec![
                Line::from(""),
                Line::from(Span::styled(
                    "😢 Installation failed!",
                    Style::default()
                        .fg(Color::Red)
                        .add_modifier(Modifier::BOLD),
                )),
                Line::from(""),
                Line::from(format!("❌ Error: {}", error_msg)),
                Line::from(""),
                Line::from("🔧 Please check the logs and try again."),
                Line::from(""),
                Line::from(Span::styled(
                    "Press Enter to exit",
                    Style::default().fg(Color::Cyan),
                )),
            ],
            Style::default().fg(Color::Red),
        )
    };

    let paragraph = Paragraph::new(text)
        .alignment(Alignment::Center)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(title)
                .border_style(style),
        );

    f.render_widget(paragraph, area);
}
