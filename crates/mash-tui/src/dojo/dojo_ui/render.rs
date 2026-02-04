use super::content::{build_dojo_lines, build_info_panel, expected_actions, status_message};
use super::sidebar::build_step_sidebar;
use super::super::dojo_app::App;
use ratatui::{
    layout::{Constraint, Direction, Layout},
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Gauge, List, ListItem, Paragraph},
    Frame,
};

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
