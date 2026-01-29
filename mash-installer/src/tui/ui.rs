//! UI rendering for the TUI

use super::app::{
    App, DownloadPhase, ImageEditionOption, ImageSource, ImageVersionOption, InstallStep,
};
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
    match app.current_step {
        InstallStep::Welcome => draw_welcome(f, app, chunks[1]),
        InstallStep::DiskSelection => draw_disk_selection(f, app, chunks[1]),
        InstallStep::DiskConfirmation => draw_disk_confirmation(f, app, chunks[1]),
        InstallStep::PartitionScheme => draw_partition_scheme(f, app, chunks[1]),
        InstallStep::PartitionLayout => draw_partition_layout(f, app, chunks[1]),
        InstallStep::PartitionCustomize => draw_partition_customize(f, app, chunks[1]),
        InstallStep::DownloadSourceSelection => draw_download_source_selection(f, app, chunks[1]),
        InstallStep::ImageSelection => draw_image_selection(f, app, chunks[1]),
        InstallStep::UefiDirectory => draw_uefi_selection(f, app, chunks[1]),
        InstallStep::LocaleSelection => draw_locale_selection(f, app, chunks[1]),
        InstallStep::Options => draw_options(f, app, chunks[1]),
        InstallStep::Confirmation => draw_confirmation(f, app, chunks[1]),
        InstallStep::DownloadingFedora => draw_downloading(f, app, chunks[1], "Fedora Image"),
        InstallStep::DownloadingUefi => draw_downloading(f, app, chunks[1], "UEFI Firmware"),
        InstallStep::Flashing => draw_progress(f, app, chunks[1]),
        InstallStep::Complete => draw_complete(f, app, chunks[1]),
    }

    // Draw help bar
    draw_help_bar(f, app, chunks[2]);
}

fn draw_title_bar(f: &mut Frame, app: &App, area: Rect) {
    let title = format!(" 🍠 MASH - {} ", app.current_step.title());

    let mode_indicator = if app.options.dry_run || app.dry_run_cli {
        " [🧪 DRY-RUN] "
    } else {
        ""
    };

    let title_block = Paragraph::new(format!("{}{}", title, mode_indicator))
        .style(
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )
        .alignment(Alignment::Center)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Cyan)),
        );

    f.render_widget(title_block, area);
}

fn draw_help_bar(f: &mut Frame, app: &App, area: Rect) {
    let help_text = match app.current_step {
        InstallStep::Welcome => "Enter: Start | Esc/q: Quit",
        InstallStep::DiskSelection => "Up/Down: Select | Enter: Confirm | r: Refresh | Esc: Back",
        InstallStep::DiskConfirmation => "Type 'DESTROY' to confirm | Esc: Back",
        InstallStep::PartitionScheme => "Up/Down: Select | Enter/Space: Confirm | Esc: Back",
        InstallStep::PartitionLayout => "Y: Use recommended | N: Customize | Esc: Back",
        InstallStep::PartitionCustomize => "Tab: Next field | Enter: Confirm | Esc: Back",
        InstallStep::DownloadSourceSelection => "Up/Down: Select | Enter: Confirm | Esc: Back",
        InstallStep::ImageSelection => match app.image_source_selection {
            ImageSource::LocalFile => {
                if app.image_input.mode == InputMode::Editing {
                    "Enter: Confirm | Esc: Cancel editing"
                } else {
                    "Enter/e/i: Edit | Tab: Next | Esc: Back"
                }
            }
            ImageSource::DownloadFedora => {
                "Up/Down: Select | Left/Right: Toggle focus | Enter/Tab: Next | Esc: Back"
            }
        },
        InstallStep::UefiDirectory => {
            if !app.download_uefi_firmware {
                if app.uefi_input.mode == InputMode::Editing {
                    "Enter: Confirm | Esc: Cancel editing | d: Toggle download"
                } else {
                    "Enter/e/i: Edit | Tab: Next | Esc: Back | d: Toggle download"
                }
            } else {
                "d: Toggle local input | Enter/Tab: Next | Esc: Back"
            }
        }
        InstallStep::LocaleSelection => "Up/Down: Select | Enter/Tab: Next | Esc: Back",
        InstallStep::Options => "Up/Down: Navigate | Space/Enter: Toggle | Tab: Next | Esc: Back",
        InstallStep::Confirmation => "Type 'FLASH' to confirm | Esc: Back",
        InstallStep::DownloadingFedora | InstallStep::DownloadingUefi => {
            if app.download_state.phase == DownloadPhase::Complete {
                "Enter: Continue"
            } else if app.download_state.phase == DownloadPhase::Failed {
                "Enter/Esc: Go back and retry"
            } else {
                "Downloading... Ctrl+C: Abort"
            }
        }
        InstallStep::Flashing => "Ctrl+C: Abort",
        InstallStep::Complete => "Enter/Esc/q: Exit",
    };

    let help = Paragraph::new(help_text)
        .style(Style::default().fg(Color::DarkGray))
        .alignment(Alignment::Center)
        .block(Block::default().borders(Borders::ALL));

    f.render_widget(help, area);
}

fn draw_welcome(f: &mut Frame, app: &App, area: Rect) {
    // Animated cursor blink
    #[allow(clippy::manual_is_multiple_of)]
    let cursor = if (app.animation_tick / 5) % 2 == 0 {
        "▌"
    } else {
        " "
    };

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
            Span::styled("Press ENTER to begin...", Style::default().fg(Color::Green)),
            Span::styled(cursor, Style::default().fg(Color::Green)),
        ]),
    ];

    let paragraph = Paragraph::new(text).alignment(Alignment::Center).block(
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

fn draw_disk_confirmation(f: &mut Frame, app: &App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(8),
            Constraint::Length(5),
            Constraint::Length(3),
        ])
        .margin(1)
        .split(area);

    // Warning message
    let disk_display = app
        .selected_disk()
        .map(|d| d.display())
        .unwrap_or_else(|| "None selected".to_string());

    let warning_text = vec![
        Line::from(Span::styled(
            "WARNING: ALL DATA WILL BE DESTROYED!",
            Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(format!("Target Disk: {}", disk_display)),
        Line::from(""),
        Line::from("This action is irreversible. All data on the disk will be lost."),
        Line::from(""),
        Line::from(Span::styled(
            "Type 'DESTROY' to confirm you understand.",
            Style::default().fg(Color::Yellow),
        )),
    ];

    let warning = Paragraph::new(warning_text)
        .alignment(Alignment::Center)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Confirm Disk Destruction ")
                .border_style(Style::default().fg(Color::Red)),
        );
    f.render_widget(warning, chunks[0]);

    // Input field
    let input_text = format!("Confirmation: {}", app.disk_confirm_input);
    let input = Paragraph::new(input_text)
        .style(Style::default().fg(Color::Yellow))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Type DESTROY ")
                .border_style(Style::default().fg(Color::Yellow)),
        );
    f.render_widget(input, chunks[1]);

    // Cursor position
    f.set_cursor_position((
        chunks[1].x + 15 + app.disk_confirm_input.len() as u16,
        chunks[1].y + 1,
    ));

    // Error message
    if let Some(ref err) = app.disk_confirm_error {
        let error = Paragraph::new(format!("Error: {}", err))
            .style(Style::default().fg(Color::Red))
            .alignment(Alignment::Center);
        f.render_widget(error, chunks[2]);
    }

    // Outer block
    let outer = Block::default()
        .borders(Borders::ALL)
        .title(" DANGER ZONE ")
        .border_style(Style::default().fg(Color::Red));
    f.render_widget(outer, area);
}

fn draw_partition_scheme(f: &mut Frame, app: &App, area: Rect) {
    let schemes = [("MBR (Recommended)", true), ("GPT (Advanced)", false)];

    let items: Vec<ListItem> = schemes
        .iter()
        .enumerate()
        .map(|(i, (label, _is_mbr))| {
            let is_selected = i == app.partition_scheme_focus;
            let style = if is_selected {
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };

            let radio = if is_selected { "(*)" } else { "( )" };
            ListItem::new(format!("  {} {}", radio, label)).style(style)
        })
        .collect();

    let help_text = vec![
        Line::from(""),
        Line::from("MBR: Traditional partition table, maximum compatibility."),
        Line::from("GPT: Modern partition table, supports larger disks."),
        Line::from(""),
        Line::from(Span::styled(
            "Recommendation: Use MBR unless you have a specific reason for GPT.",
            Style::default().fg(Color::DarkGray),
        )),
    ];

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(6), Constraint::Min(5)])
        .margin(1)
        .split(area);

    let list = List::new(items).block(
        Block::default()
            .borders(Borders::ALL)
            .title(" Select Partition Scheme ")
            .border_style(Style::default().fg(Color::Cyan)),
    );
    f.render_widget(list, chunks[0]);

    let help = Paragraph::new(help_text)
        .wrap(Wrap { trim: true })
        .block(Block::default().borders(Borders::ALL).title(" Help "));
    f.render_widget(help, chunks[1]);

    let outer = Block::default()
        .borders(Borders::ALL)
        .title(" Partition Scheme ");
    f.render_widget(outer, area);
}

fn draw_partition_layout(f: &mut Frame, app: &App, area: Rect) {
    let scheme_name = match app.partition_plan.scheme {
        crate::cli::PartitionScheme::Mbr => "MBR",
        crate::cli::PartitionScheme::Gpt => "GPT",
    };

    let layout_text = vec![
        Line::from(Span::styled(
            format!("Partition Layout ({})", scheme_name),
            Style::default().add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(format!(
            "  EFI:  {} (FAT32)",
            app.partition_plan.efi_size.display()
        )),
        Line::from(format!(
            "  BOOT: {} (ext4)",
            app.partition_plan.boot_size.display()
        )),
        Line::from(format!(
            "  ROOT: up to {} (btrfs, subvols: root,home,var)",
            app.partition_plan.root_end.display()
        )),
        Line::from("  DATA: remainder (ext4)"),
        Line::from(""),
        Line::from(""),
        Line::from(Span::styled(
            "Use recommended partition layout? (Y/n)",
            Style::default().fg(Color::Yellow),
        )),
    ];

    let paragraph = Paragraph::new(layout_text)
        .alignment(Alignment::Center)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Partition Layout ")
                .border_style(Style::default().fg(Color::Cyan)),
        );

    f.render_widget(paragraph, area);
}

fn draw_partition_customize(f: &mut Frame, app: &App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(4),
            Constraint::Length(4),
            Constraint::Length(4),
            Constraint::Length(3),
            Constraint::Min(3),
        ])
        .margin(1)
        .split(area);

    let field_names = ["EFI Size", "BOOT Size", "ROOT End"];
    let field_values = [
        app.partition_plan.efi_size.display(),
        app.partition_plan.boot_size.display(),
        app.partition_plan.root_end.display(),
    ];

    for (i, (chunk, (name, value))) in chunks
        .iter()
        .take(3)
        .zip(field_names.iter().zip(field_values.iter()))
        .enumerate()
    {
        let is_editing = i == app.partition_edit_field;
        let display_value = if is_editing {
            &app.partition_edit_input
        } else {
            value
        };

        let style = if is_editing {
            Style::default().fg(Color::Yellow)
        } else {
            Style::default()
        };

        let border_style = if is_editing {
            Style::default().fg(Color::Yellow)
        } else {
            Style::default()
        };

        let label = if is_editing {
            format!("{} (editing)", name)
        } else {
            name.to_string()
        };

        let input = Paragraph::new(display_value.clone())
            .style(style)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(format!(" {} ", label))
                    .border_style(border_style),
            );
        f.render_widget(input, *chunk);

        if is_editing {
            f.set_cursor_position((chunk.x + 1 + app.partition_edit_input.len() as u16, chunk.y + 1));
        }
    }

    // Error message
    if let Some(ref err) = app.partition_edit_error {
        let error = Paragraph::new(format!("Error: {}", err))
            .style(Style::default().fg(Color::Red))
            .alignment(Alignment::Center);
        f.render_widget(error, chunks[3]);
    }

    // Help
    let help = Paragraph::new("Use M for MiB, G for GiB (e.g., 1024M, 2G, 1800G)")
        .style(Style::default().fg(Color::DarkGray))
        .alignment(Alignment::Center)
        .block(Block::default().borders(Borders::ALL).title(" Help "));
    f.render_widget(help, chunks[4]);

    // Outer block
    let outer = Block::default()
        .borders(Borders::ALL)
        .title(" Customize Partitions ");
    f.render_widget(outer, area);
}

fn draw_download_source_selection(f: &mut Frame, app: &App, area: Rect) {
    let sources = [ImageSource::LocalFile, ImageSource::DownloadFedora];

    let items: Vec<ListItem> = sources
        .iter()
        .enumerate()
        .map(|(i, source): (usize, &ImageSource)| {
            let style = if i == app.selected_image_source_index {
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };

            let prefix = if i == app.selected_image_source_index {
                "👉 "
            } else {
                "   "
            };

            ListItem::new(format!("{}{}", prefix, source.display())).style(style)
        })
        .collect();

    let list = List::new(items).block(
        Block::default()
            .borders(Borders::ALL)
            .title(" ⬇️ Select Image Source ")
            .border_style(Style::default().fg(Color::Cyan)),
    );

    f.render_widget(list, area);
}

fn draw_image_selection(f: &mut Frame, app: &App, area: Rect) {
    match app.image_source_selection {
        ImageSource::LocalFile => {
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
        ImageSource::DownloadFedora => {
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(6), // Version list
                    Constraint::Length(6), // Edition list
                    Constraint::Min(3),    // Help text
                ])
                .margin(1)
                .split(area);

            // Version selection
            let version_items: Vec<ListItem> = ImageVersionOption::all()
                .iter()
                .enumerate()
                .map(|(i, version): (usize, &ImageVersionOption)| {
                    let style = if i == app.selected_image_version_index && app.options_focus == 0 {
                        Style::default()
                            .fg(Color::Yellow)
                            .add_modifier(Modifier::BOLD)
                    } else {
                        Style::default()
                    };
                    let prefix = if i == app.selected_image_version_index && app.options_focus == 0
                    {
                        "👉 "
                    } else {
                        "   "
                    };
                    ListItem::new(format!("{}{}", prefix, version.display())).style(style)
                })
                .collect();

            let version_list = List::new(version_items).block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(" 🚀 Select Fedora Version ")
                    .border_style(if app.options_focus == 0 {
                        Style::default().fg(Color::Yellow)
                    } else {
                        Style::default().fg(Color::DarkGray)
                    }),
            );
            f.render_widget(version_list, chunks[0]);

            // Edition selection
            let edition_items: Vec<ListItem> = ImageEditionOption::all()
                .iter()
                .enumerate()
                .map(|(i, edition): (usize, &ImageEditionOption)| {
                    let style = if i == app.selected_image_edition_index && app.options_focus == 1 {
                        Style::default()
                            .fg(Color::Yellow)
                            .add_modifier(Modifier::BOLD)
                    } else {
                        Style::default()
                    };
                    let prefix = if i == app.selected_image_edition_index && app.options_focus == 1
                    {
                        "👉 "
                    } else {
                        "   "
                    };
                    ListItem::new(format!("{}{}", prefix, edition.display())).style(style)
                })
                .collect();

            let edition_list = List::new(edition_items).block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(" 🖥️ Select Fedora Edition ")
                    .border_style(if app.options_focus == 1 {
                        Style::default().fg(Color::Yellow)
                    } else {
                        Style::default().fg(Color::DarkGray)
                    }),
            );
            f.render_widget(edition_list, chunks[1]);

            // Help text
            let help = Paragraph::new(
                "↑↓: Select option | ←→: Toggle focus between Version/Edition\n\
                 ⏎/Tab: Next | Esc: Back\n\
                 Images will be downloaded to 'mash_root/downloads/images'.",
            )
            .wrap(Wrap { trim: true })
            .block(Block::default().borders(Borders::ALL).title(" 💡 Help "));

            f.render_widget(help, chunks[2]);

            // Outer block
            let outer = Block::default()
                .borders(Borders::ALL)
                .title(" 🌐 Download Fedora Image ");
            f.render_widget(outer, area);
        }
    }
}

fn draw_uefi_selection(f: &mut Frame, app: &App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Input/Download option
            Constraint::Length(3), // Error message
            Constraint::Min(3),    // Help text
        ])
        .margin(1)
        .split(area);

    // UEFI Input / Download option
    let uefi_option_widget = if !app.download_uefi_firmware {
        // Input field for local UEFI directory
        let input_style = if app.uefi_input.mode == InputMode::Editing {
            Style::default().fg(Color::Yellow)
        } else {
            Style::default()
        };

        Paragraph::new(app.uefi_input.value())
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
            )
    } else {
        // Displaying download option
        let checkbox = CheckboxState::from(app.download_uefi_firmware);
        Paragraph::new(format!("{} Download UEFI firmware", checkbox.symbol()))
            .style(Style::default().fg(Color::Green))
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(" ⬇️ UEFI Firmware Source ")
                    .border_style(Style::default().fg(Color::Green)),
            )
    };
    f.render_widget(uefi_option_widget, chunks[0]);

    // Show cursor when editing, only if not downloading
    if !app.download_uefi_firmware && app.uefi_input.mode == InputMode::Editing {
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
    let help = if !app.download_uefi_firmware {
        Paragraph::new(
            "📁 Enter the directory containing UEFI files for Raspberry Pi 4.\n\
             🎩 These will be copied onto the EFI partition.\n\
             Press 'd' to toggle downloading UEFI firmware from GitHub.",
        )
    } else {
        Paragraph::new(
            "⬇️ UEFI firmware will be downloaded from GitHub to 'mash_root/downloads/uefi'.\n\
             Press 'd' to toggle entering a local UEFI directory.",
        )
    }
    .wrap(Wrap { trim: true })
    .block(Block::default().borders(Borders::ALL).title(" 💡 Help "));

    f.render_widget(help, chunks[2]);

    // Outer block
    let outer_title = if app.download_uefi_firmware {
        " 🌐 Download UEFI Firmware "
    } else {
        " 🔧 UEFI Configuration "
    };
    let outer_border_style = if app.download_uefi_firmware {
        Style::default().fg(Color::Green)
    } else {
        Style::default()
    };
    let outer = Block::default()
        .borders(Borders::ALL)
        .title(outer_title)
        .border_style(outer_border_style);
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
        if matches!(
            app.options.partition_scheme,
            crate::cli::PartitionScheme::Mbr
        ) {
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

            let prefix = if i == app.options_focus {
                "👉 "
            } else {
                "   "
            };

            let symbol = match checked_opt {
                Some(checked) => CheckboxState::from(*checked).symbol().to_string(),
                None => "🔁".to_string(),
            };

            let text = format!(
                "{}{} {}
      📝 {}",
                prefix, symbol, label, desc
            );
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
            Constraint::Min(10),   // Summary
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
            Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(format!("💾 Target Disk: {}", disk_display)),
        Line::from(format!("📀 Image: {}", app.image_input.value())),
        Line::from(format!("🔧 UEFI Dir: {}", app.uefi_input.value())),
        Line::from(format!("🌍 Locale: {}", locale_display)),
        Line::from(format!(
            "🧭 Partition Scheme: {}",
            app.options.partition_scheme
        )),
        Line::from(format!(
            "🔐 Early SSH: {}",
            if app.options.early_ssh {
                "✅ Yes"
            } else {
                "❌ No"
            }
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

fn draw_downloading(f: &mut Frame, app: &App, area: Rect, download_type: &str) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(5), // Title/description
            Constraint::Length(5), // Progress bar
            Constraint::Length(5), // Stats
            Constraint::Min(5),    // Status/messages
        ])
        .margin(1)
        .split(area);

    // Title and description
    let description = match app.download_state.phase {
        DownloadPhase::NotStarted => format!("⏳ Preparing to download {}...", download_type),
        DownloadPhase::Downloading => {
            if app.download_state.description.is_empty() {
                format!("📥 Downloading {}...", download_type)
            } else {
                app.download_state.description.clone()
            }
        }
        DownloadPhase::Extracting => format!("📦 Extracting {}...", download_type),
        DownloadPhase::Complete => format!("✅ {} download complete!", download_type),
        DownloadPhase::Failed => format!("❌ {} download failed!", download_type),
    };

    let title_style = match app.download_state.phase {
        DownloadPhase::Complete => Style::default()
            .fg(Color::Green)
            .add_modifier(Modifier::BOLD),
        DownloadPhase::Failed => Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
        _ => Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD),
    };

    let title_block = Paragraph::new(description)
        .style(title_style)
        .alignment(Alignment::Center)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(format!(" 📥 Downloading {} ", download_type)),
        );
    f.render_widget(title_block, chunks[0]);

    // Progress bar
    let (percent, progress_label) = if let Some(total) = app.download_state.total_bytes {
        let pct = if total > 0 {
            ((app.download_state.current_bytes as f64 / total as f64) * 100.0) as u16
        } else {
            0
        };
        let label = format!(
            "{}% - {} / {}",
            pct,
            format_bytes(app.download_state.current_bytes),
            format_bytes(total)
        );
        (pct, label)
    } else {
        let label = format!(
            "Downloaded: {}",
            format_bytes(app.download_state.current_bytes)
        );
        (0, label)
    };

    let gauge_style = match app.download_state.phase {
        DownloadPhase::Complete => Style::default().fg(Color::Green),
        DownloadPhase::Failed => Style::default().fg(Color::Red),
        DownloadPhase::Extracting => Style::default().fg(Color::Yellow),
        _ => Style::default().fg(Color::Cyan),
    };

    let gauge = Gauge::default()
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" 📊 Progress "),
        )
        .gauge_style(gauge_style)
        .percent(percent.min(100))
        .label(progress_label);
    f.render_widget(gauge, chunks[1]);

    // Stats panel
    let speed_str = format_bytes(app.download_state.speed_bytes_per_sec);
    let eta_str = if app.download_state.eta_seconds > 0 {
        format!("{}s", app.download_state.eta_seconds)
    } else {
        "calculating...".to_string()
    };

    let stats_text = vec![
        Line::from(format!("⚡ Speed: {}/s", speed_str)),
        Line::from(format!("🎯 ETA: {}", eta_str)),
    ];

    let stats = Paragraph::new(stats_text)
        .alignment(Alignment::Center)
        .block(Block::default().borders(Borders::ALL).title(" 📈 Stats "));
    f.render_widget(stats, chunks[2]);

    // Status/error messages
    let status_content = if let Some(ref err) = app.download_state.error {
        vec![
            Line::from(""),
            Line::from(Span::styled(
                format!("❌ Error: {}", err),
                Style::default().fg(Color::Red),
            )),
            Line::from(""),
            Line::from("Press Enter or Esc to go back and retry."),
        ]
    } else if app.download_state.phase == DownloadPhase::Complete {
        vec![
            Line::from(""),
            Line::from(Span::styled(
                "✅ Download complete!",
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD),
            )),
            Line::from(""),
            Line::from("Press Enter to continue..."),
        ]
    } else {
        // Animated spinner during download
        let spinner = match (app.animation_tick / 3) % 4 {
            0 => "⠋",
            1 => "⠙",
            2 => "⠹",
            _ => "⠸",
        };
        vec![
            Line::from(""),
            Line::from(format!("{} Downloading... Please wait.", spinner)),
            Line::from(""),
            Line::from(Span::styled(
                "⚠️  Large files may take several minutes",
                Style::default().fg(Color::Yellow),
            )),
        ]
    };

    let status = Paragraph::new(status_content)
        .alignment(Alignment::Center)
        .block(Block::default().borders(Borders::ALL).title(" 📝 Status "));
    f.render_widget(status, chunks[3]);

    // Outer block
    let outer_style = match app.download_state.phase {
        DownloadPhase::Complete => Style::default().fg(Color::Green),
        DownloadPhase::Failed => Style::default().fg(Color::Red),
        _ => Style::default().fg(Color::Cyan),
    };
    let outer = Block::default()
        .borders(Borders::ALL)
        .title(format!(" 📥 {} Download ", download_type))
        .border_style(outer_style);
    f.render_widget(outer, area);
}

/// Format bytes into human-readable string
fn format_bytes(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;

    if bytes >= GB {
        format!("{:.2} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.1} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.1} KB", bytes as f64 / KB as f64)
    } else {
        format!("{} B", bytes)
    }
}

fn draw_progress(f: &mut Frame, app: &App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Overall progress bar
            Constraint::Min(12),   // Analytics + Phases (split horizontally)
            Constraint::Length(3), // Status
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
            app.progress.current_phase.map(|p| p.number()).unwrap_or(0),
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
        Line::from(format!("🚀 Peak:      {:.1} MB/s", app.progress.peak_speed)),
        Line::from(""),
        Line::from(Span::styled(
            "⏱️ Time",
            Style::default().add_modifier(Modifier::BOLD),
        )),
        Line::from("─────────────────────"),
        Line::from(format!("⏳ Elapsed:   {}", app.progress.elapsed_string())),
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

    let analytics = Paragraph::new(analytics_lines).block(
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
        let error_msg = app.install_error.as_deref().unwrap_or("Unknown error");
        (
            " ❌ Installation Failed ".to_string(),
            vec![
                Line::from(""),
                Line::from(Span::styled(
                    "😢 Installation failed!",
                    Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
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

    let paragraph = Paragraph::new(text).alignment(Alignment::Center).block(
        Block::default()
            .borders(Borders::ALL)
            .title(title)
            .border_style(style),
    );

    f.render_widget(paragraph, area);
}
