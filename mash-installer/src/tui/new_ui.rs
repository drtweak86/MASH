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

    // Current Step Display
    let current_step_title = app.current_step_type.title();
    let current_step_item = ListItem::new(format!(">> {}", current_step_title));
    let list = List::new(vec![current_step_item]);
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
