//! New UI module for the single-screen TUI

use crate::tui::new_app::{App, InstallStepType};
use crate::tui::progress::{Phase, ProgressState};
use ratatui::{
    layout::{Constraint, Direction, Layout},
    style::{Color, Style},
    widgets::{Block, Borders, Gauge, List, ListItem, Paragraph},
    Frame,
};

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
    let title = Block::default()
        .borders(Borders::ALL)
        .title("MASH Installer");
    f.render_widget(title, chunks[0]);

    // Current Step Display
    let current_step_title = app.current_step_type.title();
    let mut items = Vec::new();
    items.push(ListItem::new(format!("🧭 Step: {}", current_step_title)));

    match app.current_step_type {
        InstallStepType::DiskSelection => {
            items.push(ListItem::new("💽 Disk list not available yet.".to_string()));
            items.push(ListItem::new(
                "ℹ️ Placeholder: Select Target Disk options will render here.".to_string(),
            ));
            items.push(ListItem::new(
                "⌨️ Use Enter to continue for now.".to_string(),
            ));
        }
        InstallStepType::BackupConfirmation => {
            items.push(ListItem::new(
                "⚠️ This will erase data on the selected disk.".to_string(),
            ));
            items.push(ListItem::new(
                "💾 Have you backed up your data? (Y/N)".to_string(),
            ));
            if app.backup_confirmed {
                items.push(ListItem::new("✅ Backup confirmed.".to_string()));
            }
        }
        InstallStepType::FirstBootUser => {
            items.push(ListItem::new(
                "🧑‍💻 First boot will prompt you to create a user.".to_string(),
            ));
            items.push(ListItem::new(
                "🔐 Autologin will be disabled for safety.".to_string(),
            ));
            items.push(ListItem::new("ℹ️ Press Enter to continue.".to_string()));
        }
        _ => {
            items.push(ListItem::new(
                "ℹ️ Enter to continue - Esc to go back - q to quit".to_string(),
            ));
        }
    }

    if let Some(error) = &app.error_message {
        items.push(ListItem::new(format!("❌ {}", error)));
    }
    let list = List::new(items).block(Block::default().borders(Borders::ALL).title("Wizard"));
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

fn status_message(app: &App, progress_state: &ProgressState) -> String {
    let message = if !progress_state.status.is_empty() {
        progress_state.status.clone()
    } else {
        app.status_message.clone()
    };
    ensure_emoji_prefix(message)
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
