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
    let wizard_lines = build_wizard_lines(app);
    let list_items = wizard_lines
        .into_iter()
        .map(ListItem::new)
        .collect::<Vec<_>>();
    let list = List::new(list_items).block(Block::default().borders(Borders::ALL).title("Wizard"));
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
    let wizard_lines = build_wizard_lines(app);
    let header = "MASH Installer";
    let wizard_hint = wizard_lines
        .first()
        .cloned()
        .unwrap_or_else(|| "🧭 Step: (unknown)".to_string());
    let body_lines = if wizard_lines.len() > 1 {
        wizard_lines[1..].join("\n")
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
        "STEP: {}\n\n- Header: {}\n- Wizard hint line: {}\n- Body contents:\n{}\n- Footer/progress/telemetry/status blocks:\nProgress: {}%\nTelemetry: {}\nStatus: {}\n- Expected user actions (keys): {}\n",
        app.current_step_type.title(),
        header,
        wizard_hint,
        body_lines,
        percent,
        telemetry,
        status,
        actions
    )
}

fn build_wizard_lines(app: &App) -> Vec<String> {
    let current_step_title = app.current_step_type.title();
    let mut items = Vec::new();
    items.push(format!("🧭 Step: {}", current_step_title));

    match app.current_step_type {
        InstallStepType::Welcome => {
            items.push("👋 Welcome screen content not loaded yet.".to_string());
            items.push("ℹ️ Expected from static copy in wizard config.".to_string());
            items.push("⌨️ Press Enter to begin.".to_string());
        }
        InstallStepType::DiskSelection => {
            items.push("💽 Disk list not available yet.".to_string());
            items.push("ℹ️ Placeholder: Select Target Disk options will render here.".to_string());
            items.push("⌨️ Use Enter to continue for now.".to_string());
        }
        InstallStepType::DiskConfirmation => {
            items.push("⚠️ No target disk selected yet.".to_string());
            items.push("ℹ️ Expected from disk scan selection in DiskSelection.".to_string());
            items.push("⌨️ Confirm disk choice will render here.".to_string());
        }
        InstallStepType::BackupConfirmation => {
            items.push("⚠️ This will erase data on the selected disk.".to_string());
            items.push("💾 Have you backed up your data? (Y/N)".to_string());
            if app.backup_confirmed {
                items.push("✅ Backup confirmed.".to_string());
            }
        }
        InstallStepType::PartitionScheme => {
            items.push("🧩 Partition schemes not loaded yet.".to_string());
            items.push("ℹ️ Expected from defaults or user configuration.".to_string());
            items.push("⌨️ Scheme options will render here.".to_string());
        }
        InstallStepType::PartitionLayout => {
            items.push("📐 Partition layout not calculated yet.".to_string());
            items.push("ℹ️ Expected from selected scheme and disk size.".to_string());
            items.push("⌨️ Layout preview will render here.".to_string());
        }
        InstallStepType::PartitionCustomize => {
            items.push("🛠️ Custom partition options not loaded yet.".to_string());
            items.push("ℹ️ Expected from partition layout details.".to_string());
            items.push("⌨️ Customization controls will render here.".to_string());
        }
        InstallStepType::DownloadSourceSelection => {
            items.push("📥 Image sources not loaded yet.".to_string());
            items.push("ℹ️ Expected from defaults or download configuration.".to_string());
            items.push("⌨️ Source options will render here.".to_string());
        }
        InstallStepType::ImageSelection => {
            items.push("🖼️ Image list not loaded yet.".to_string());
            items.push("ℹ️ Expected from download list or local file picker.".to_string());
            items.push("⌨️ Image selection options will render here.".to_string());
        }
        InstallStepType::UefiDirectory => {
            items.push("📁 UEFI directory not set yet.".to_string());
            items.push("ℹ️ Expected from local directory selection or download.".to_string());
            items.push("⌨️ UEFI directory picker will render here.".to_string());
        }
        InstallStepType::LocaleSelection => {
            items.push("🗣️ Locale options not loaded yet.".to_string());
            items.push("ℹ️ Expected from locale defaults or system list.".to_string());
            items.push("⌨️ Locale and keymap options will render here.".to_string());
        }
        InstallStepType::Options => {
            items.push("⚙️ Installation options not loaded yet.".to_string());
            items.push("ℹ️ Expected from defaults and user selections.".to_string());
            items.push("⌨️ Option toggles will render here.".to_string());
        }
        InstallStepType::FirstBootUser => {
            items.push("🧑‍💻 First boot will prompt you to create a user.".to_string());
            items.push("🔐 Autologin will be disabled for safety.".to_string());
            items.push("ℹ️ Press Enter to continue.".to_string());
        }
        InstallStepType::Confirmation => {
            items.push("✅ Confirmation summary not built yet.".to_string());
            items.push("ℹ️ Expected from selected disk, image, and options.".to_string());
            items.push("⌨️ Final confirmation details will render here.".to_string());
        }
        InstallStepType::DownloadingFedora => {
            items.push("⬇️ Download progress not available yet.".to_string());
            items.push("ℹ️ Expected from downloader telemetry.".to_string());
            items.push("⌨️ Download status will render here.".to_string());
        }
        InstallStepType::DownloadingUefi => {
            items.push("⬇️ UEFI download progress not available yet.".to_string());
            items.push("ℹ️ Expected from downloader telemetry.".to_string());
            items.push("⌨️ Download status will render here.".to_string());
        }
        InstallStepType::Flashing => {
            items.push("💾 Flashing progress is shown below.".to_string());
            items.push("ℹ️ Live telemetry expected from flash.rs progress updates.".to_string());
            items.push("⌨️ Press Enter when complete.".to_string());
        }
        InstallStepType::Complete => {
            items.push("🎉 Installation complete.".to_string());
            items.push("ℹ️ Final summary will render here.".to_string());
            items.push("⌨️ Press Enter to exit.".to_string());
        }
    }

    if let Some(error) = &app.error_message {
        items.push(format!("❌ {}", error));
    }

    items
}

fn expected_actions(step: InstallStepType) -> String {
    match step {
        InstallStepType::BackupConfirmation => "Y/N, Esc, q".to_string(),
        InstallStepType::Flashing => "Enter when complete, q".to_string(),
        InstallStepType::Complete => "Enter to exit".to_string(),
        InstallStepType::DownloadingFedora | InstallStepType::DownloadingUefi => {
            "Wait, q".to_string()
        }
        _ => "Enter, Esc, q".to_string(),
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
