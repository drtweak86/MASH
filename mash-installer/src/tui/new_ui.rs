//! New UI module for the single-screen TUI

use crate::tui::new_app::App;
use ratatui::{
    layout::{Constraint, Direction, Layout},
    style::{Color, Style},
    widgets::{Block, Borders, Gauge, List, ListItem},
    Frame,
};

pub fn draw(f: &mut Frame, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints(
            [
                Constraint::Length(3), // Title
                Constraint::Min(0),    // Steps
                Constraint::Length(3), // Progress bar
                Constraint::Length(1), // Status line
            ]
            .as_ref(),
        )
        .split(f.area());

    // Title
    let title = Block::default()
        .borders(Borders::ALL)
        .title("MASH Installer");
    f.render_widget(title, chunks[0]);

    // Steps
    let items: Vec<ListItem> = app
        .steps
        .iter()
        .map(|step| {
            let state_symbol = match step.state {
                super::new_app::StepState::Pending => "[ ]",
                super::new_app::StepState::Running => "[>]",
                super::new_app::StepState::Completed => "[✓]",
                super::new_app::StepState::Failed => "[✗]",
                super::new_app::StepState::Skipped => "[-]",
            };
            ListItem::new(format!("{} {}", state_symbol, step.name))
        })
        .collect();
    let list = List::new(items);
    f.render_widget(list, chunks[1]);

    // Progress bar
    let gauge = Gauge::default()
        .block(Block::default().borders(Borders::ALL).title("Progress"))
        .gauge_style(Style::default().fg(Color::Yellow))
        .percent(50); // Placeholder
    f.render_widget(gauge, chunks[2]);

    // Status line
    let status_message = app.status_message.as_str();
    f.render_widget(Block::default().title(status_message), chunks[3]);
}
