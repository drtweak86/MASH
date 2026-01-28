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
        " MASH Installer - {} ",
        app.current_screen.title()
    );

    let mode_indicator = if app.options.dry_run || app.dry_run_cli {
        " [DRY-RUN] "
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
        Screen::Welcome => "Enter: Start | Esc/q: Quit",
        Screen::DiskSelection => "Up/Down: Select | Enter: Confirm | r: Refresh | Esc: Back",
        Screen::ImageSelection => {
            if app.image_input.mode == InputMode::Editing {
                "Enter: Confirm | Esc: Cancel editing"
            } else {
                "Enter/e/i: Edit | Tab: Next | Esc: Back"
            }
        }
        Screen::UefiDirectory => {
            if app.uefi_input.mode == InputMode::Editing {
                "Enter: Confirm | Esc: Cancel editing"
            } else {
                "Enter/e/i: Edit | Tab: Next | Esc: Back"
            }
        }
        Screen::LocaleSelection => "Up/Down: Select | Enter/Tab: Next | Esc: Back",
        Screen::Options => "Up/Down: Navigate | Space/Enter: Toggle | Tab: Next | Esc: Back",
        Screen::Confirmation => "Type 'YES I KNOW' to confirm | Esc: Back",
        Screen::Progress => "Ctrl+C: Abort",
        Screen::Complete => "Enter/Esc/q: Exit",
    };

    let help = Paragraph::new(help_text)
        .style(Style::default().fg(Color::DarkGray))
        .alignment(Alignment::Center)
        .block(Block::default().borders(Borders::ALL));

    f.render_widget(help, area);
}

fn draw_welcome(f: &mut Frame, _app: &App, area: Rect) {
    let text = vec![
        Line::from(""),
        Line::from(""),
        Line::from(Span::styled(
            "Enter the Dojo",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(""),
        Line::from("MASH Installer will guide you through:"),
        Line::from(""),
        Line::from("  1. Select target disk (will be ERASED)"),
        Line::from("  2. Select Fedora image file"),
        Line::from("  3. Configure UEFI overlay"),
        Line::from("  4. Choose locale and keymap"),
        Line::from("  5. Set installation options"),
        Line::from("  6. Flash the image"),
        Line::from(""),
        Line::from(""),
        Line::from(Span::styled(
            "Press ENTER to begin...",
            Style::default().fg(Color::Green),
        )),
    ];

    let paragraph = Paragraph::new(text)
        .alignment(Alignment::Center)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Welcome ")
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
                "> "
            } else {
                "  "
            };

            ListItem::new(format!("{}{}", prefix, disk.display())).style(style)
        })
        .collect();

    let list = List::new(items).block(
        Block::default()
            .borders(Borders::ALL)
            .title(" Select Target Disk (will be ERASED!) ")
            .border_style(Style::default().fg(Color::Red)),
    );

    if app.available_disks.is_empty() {
        let no_disks = Paragraph::new("No removable disks found.\n\nPress 'r' to refresh.")
            .alignment(Alignment::Center)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(" Select Target Disk ")
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
                    " {} {} ",
                    app.image_input.placeholder,
                    if app.image_input.mode == InputMode::Editing {
                        "(editing)"
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
        let error = Paragraph::new(err.as_str())
            .style(Style::default().fg(Color::Red))
            .block(Block::default().borders(Borders::ALL).title(" Error "));
        f.render_widget(error, chunks[1]);
    }

    // Help text
    let help = Paragraph::new(
        "Enter the full path to a Fedora .raw image file.\n\
         The image will be loop-mounted and copied to the target disk.",
    )
    .wrap(Wrap { trim: true })
    .block(Block::default().borders(Borders::ALL).title(" Help "));

    f.render_widget(help, chunks[2]);

    // Outer block
    let outer = Block::default()
        .borders(Borders::ALL)
        .title(" Select Image File ");
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
                    " {} {} ",
                    app.uefi_input.placeholder,
                    if app.uefi_input.mode == InputMode::Editing {
                        "(editing)"
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
        let error = Paragraph::new(err.as_str())
            .style(Style::default().fg(Color::Red))
            .block(Block::default().borders(Borders::ALL).title(" Error "));
        f.render_widget(error, chunks[1]);
    }

    // Help text
    let help = Paragraph::new(
        "Directory containing UEFI files for Raspberry Pi 4.\n\
         These will be copied onto the EFI partition.",
    )
    .wrap(Wrap { trim: true })
    .block(Block::default().borders(Borders::ALL).title(" Help "));

    f.render_widget(help, chunks[2]);

    // Outer block
    let outer = Block::default()
        .borders(Borders::ALL)
        .title(" UEFI Directory ");
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
                "> "
            } else {
                "  "
            };

            ListItem::new(format!(
                "{}{} (keymap: {})",
                prefix, locale.lang, locale.keymap
            ))
            .style(style)
        })
        .collect();

    let list = List::new(items).block(
        Block::default()
            .borders(Borders::ALL)
            .title(" Select Locale & Keymap ")
            .border_style(Style::default().fg(Color::Cyan)),
    );

    f.render_widget(list, area);
}

fn draw_options(f: &mut Frame, app: &App, area: Rect) {
    let options = [
        (
            "Auto-unmount target disk mounts",
            app.options.auto_unmount,
            "Automatically unmount any partitions from the target disk",
        ),
        (
            "Enable Early SSH",
            app.options.early_ssh,
            "Enable SSH access before graphical login (recommended)",
        ),
        (
            "Dry-run mode",
            app.options.dry_run,
            "Print what would happen without making changes",
        ),
    ];

    let items: Vec<ListItem> = options
        .iter()
        .enumerate()
        .map(|(i, (label, checked, desc))| {
            let checkbox = CheckboxState::from(*checked);
            let style = if i == app.options_focus {
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };

            let prefix = if i == app.options_focus { "> " } else { "  " };

            let text = format!("{}{} {}\n      {}", prefix, checkbox.symbol(), label, desc);
            ListItem::new(text).style(style)
        })
        .collect();

    let list = List::new(items).block(
        Block::default()
            .borders(Borders::ALL)
            .title(" Installation Options ")
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
            "DANGER ZONE",
            Style::default()
                .fg(Color::Red)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(format!("Target Disk: {}", disk_display)),
        Line::from(format!("Image: {}", app.image_input.value())),
        Line::from(format!("UEFI Dir: {}", app.uefi_input.value())),
        Line::from(format!("Locale: {}", locale_display)),
        Line::from(format!(
            "Early SSH: {}",
            if app.options.early_ssh { "Yes" } else { "No" }
        )),
        Line::from(format!(
            "Dry-run: {}",
            if app.options.dry_run || app.dry_run_cli {
                "Yes"
            } else {
                "No"
            }
        )),
        Line::from(""),
        Line::from(Span::styled(
            "This will ERASE the target disk!",
            Style::default().fg(Color::Red),
        )),
    ];

    let summary = Paragraph::new(summary_text)
        .alignment(Alignment::Center)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Installation Summary ")
                .border_style(Style::default().fg(Color::Red)),
        );

    f.render_widget(summary, chunks[0]);

    // Confirmation input
    let input_text = format!(
        "Type 'YES I KNOW' to confirm: {}",
        app.confirmation_input
    );
    let input = Paragraph::new(input_text)
        .style(Style::default().fg(Color::Yellow))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Confirmation ")
                .border_style(Style::default().fg(Color::Yellow)),
        );

    f.render_widget(input, chunks[1]);

    // Set cursor position
    f.set_cursor_position((
        chunks[1].x + 35 + app.confirmation_input.len() as u16,
        chunks[1].y + 1,
    ));

    // Error
    if let Some(ref err) = app.confirmation_error {
        let error = Paragraph::new(err.as_str())
            .style(Style::default().fg(Color::Red))
            .alignment(Alignment::Center);
        f.render_widget(error, chunks[2]);
    }

    // Outer block
    let outer = Block::default()
        .borders(Borders::ALL)
        .title(" Confirm Installation ")
        .border_style(Style::default().fg(Color::Red));
    f.render_widget(outer, area);
}

fn draw_progress(f: &mut Frame, app: &App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),  // Overall progress
            Constraint::Length(5),  // Current phase info
            Constraint::Min(10),    // Phase list
            Constraint::Length(3),  // Status
        ])
        .margin(1)
        .split(area);

    // Overall progress bar
    let progress_label = format!(
        "{}% - ETA: {}",
        app.progress.overall_percent as u32,
        app.progress.eta_string()
    );
    let gauge = Gauge::default()
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Overall Progress "),
        )
        .gauge_style(Style::default().fg(Color::Green))
        .percent(app.progress.overall_percent as u16)
        .label(progress_label);

    f.render_widget(gauge, chunks[0]);

    // Current phase info
    let phase_info = if let Some(phase) = app.progress.current_phase {
        let speed_text = if app.progress.rsync_speed > 0.0 {
            format!("Speed: {:.1} MB/s", app.progress.rsync_speed)
        } else if app.progress.disk_io_speed > 0.0 {
            format!("Disk I/O: {:.1} MB/s", app.progress.disk_io_speed)
        } else {
            String::new()
        };

        let files_text = if app.progress.files_total > 0 {
            format!(
                "Files: {} / {}",
                app.progress.files_done, app.progress.files_total
            )
        } else {
            String::new()
        };

        vec![
            Line::from(format!("Current: {}", phase.name())),
            Line::from(speed_text),
            Line::from(files_text),
        ]
    } else {
        vec![Line::from("Preparing...")]
    };

    let phase_para = Paragraph::new(phase_info).block(
        Block::default()
            .borders(Borders::ALL)
            .title(" Current Phase "),
    );

    f.render_widget(phase_para, chunks[1]);

    // Phase list
    let phase_items: Vec<ListItem> = Phase::all()
        .iter()
        .map(|phase| {
            let symbol = app.progress.phase_symbol(*phase);
            let style = if app.progress.current_phase == Some(*phase) {
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD)
            } else if app.progress.completed_phases.contains(phase) {
                Style::default().fg(Color::Green)
            } else {
                Style::default().fg(Color::DarkGray)
            };

            ListItem::new(format!("  {} Phase {}: {}", symbol, phase.number(), phase.name()))
                .style(style)
        })
        .collect();

    let phase_list = List::new(phase_items).block(
        Block::default()
            .borders(Borders::ALL)
            .title(" Installation Phases "),
    );

    f.render_widget(phase_list, chunks[2]);

    // Status message
    let status_style = if app.progress.error.is_some() {
        Style::default().fg(Color::Red)
    } else {
        Style::default().fg(Color::Cyan)
    };

    let status = Paragraph::new(app.progress.status.as_str())
        .style(status_style)
        .alignment(Alignment::Center)
        .block(Block::default().borders(Borders::ALL).title(" Status "));

    f.render_widget(status, chunks[3]);

    // Outer block
    let outer = Block::default()
        .borders(Borders::ALL)
        .title(format!(
            " Installing - Phase {}/{} ",
            app.progress
                .current_phase
                .map(|p| p.number())
                .unwrap_or(0),
            Phase::total()
        ))
        .border_style(Style::default().fg(Color::Yellow));
    f.render_widget(outer, area);
}

fn draw_complete(f: &mut Frame, app: &App, area: Rect) {
    let (title, text, style) = if app.install_success {
        (
            " Installation Complete ",
            vec![
                Line::from(""),
                Line::from(Span::styled(
                    "Installation completed successfully!",
                    Style::default()
                        .fg(Color::Green)
                        .add_modifier(Modifier::BOLD),
                )),
                Line::from(""),
                Line::from("Next steps:"),
                Line::from(""),
                Line::from("  1. Remove the disk from this computer"),
                Line::from("  2. Insert into your Raspberry Pi 4"),
                Line::from("  3. Boot with UEFI"),
                Line::from("  4. Run Dojo setup: sudo /data/mash-staging/install_dojo.sh"),
                Line::from(""),
                Line::from(""),
                Line::from(Span::styled(
                    "Press Enter to exit",
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
            " Installation Failed ",
            vec![
                Line::from(""),
                Line::from(Span::styled(
                    "Installation failed!",
                    Style::default()
                        .fg(Color::Red)
                        .add_modifier(Modifier::BOLD),
                )),
                Line::from(""),
                Line::from(format!("Error: {}", error_msg)),
                Line::from(""),
                Line::from("Please check the logs and try again."),
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
