use crossterm::event::{self, Event, KeyCode};
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use crossterm::execute;
use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem, Paragraph};
use ratatui::{backend::CrosstermBackend, Terminal};
use std::fs;
use std::io::{self, Stdout};
use std::process::Command;
use std::time::{Duration, SystemTime};

const MARKER_PATH: &str = "/var/lib/mash/dojo.completed";
const SERVICE_NAME: &str = "mash-dojo.service";

struct App {
    items: Vec<&'static str>,
    selected: usize,
    status: String,
}

impl App {
    fn new() -> Self {
        Self {
            items: vec!["Show quick tips", "Continue to desktop"],
            selected: 0,
            status: "Welcome to MASH Dojo.".to_string(),
        }
    }

    fn select_next(&mut self) {
        if self.items.is_empty() {
            self.selected = 0;
            return;
        }
        self.selected = (self.selected + 1) % self.items.len();
    }

    fn select_prev(&mut self) {
        if self.items.is_empty() {
            self.selected = 0;
            return;
        }
        if self.selected == 0 {
            self.selected = self.items.len() - 1;
        } else {
            self.selected -= 1;
        }
    }
}

fn main() -> anyhow::Result<()> {
    if std::path::Path::new(MARKER_PATH).exists() {
        return Ok(());
    }

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let result = run(&mut terminal);

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    result
}

fn run(terminal: &mut Terminal<CrosstermBackend<Stdout>>) -> anyhow::Result<()> {
    let mut app = App::new();

    loop {
        terminal.draw(|f| draw(f, &app))?;

        if event::poll(Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                match key.code {
                    KeyCode::Up => app.select_prev(),
                    KeyCode::Down => app.select_next(),
                    KeyCode::Enter => match app.selected {
                        0 => {
                            app.status =
                                "Tip: Use the MASH TUI to re-run installs safely.".to_string();
                        }
                        1 => {
                            finalize_dojo()?;
                            return Ok(());
                        }
                        _ => {}
                    },
                    KeyCode::Char('q') => {
                        return Ok(());
                    }
                    _ => {}
                }
            }
        }
    }
}

fn draw(f: &mut ratatui::Frame, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints(
            [
                Constraint::Length(3),
                Constraint::Min(0),
                Constraint::Length(3),
            ]
            .as_ref(),
        )
        .split(f.area());

    let title = Line::from(vec![
        Span::styled("MASH Dojo", Style::default().fg(Color::Yellow)),
        Span::raw(" — First Boot"),
    ]);
    let header = Block::default().borders(Borders::ALL).title(title);
    f.render_widget(header, chunks[0]);

    let items = app
        .items
        .iter()
        .enumerate()
        .map(|(idx, item)| {
            let marker = if idx == app.selected { "▶" } else { " " };
            ListItem::new(format!("{} {}", marker, item))
        })
        .collect::<Vec<_>>();
    let list = List::new(items).block(Block::default().borders(Borders::ALL).title("Menu"));
    f.render_widget(list, chunks[1]);

    let help = Paragraph::new(app.status.as_str())
        .block(Block::default().borders(Borders::ALL).title("Status"));
    f.render_widget(help, chunks[2]);
}

fn finalize_dojo() -> anyhow::Result<()> {
    let marker_dir = std::path::Path::new("/var/lib/mash");
    fs::create_dir_all(marker_dir)?;

    let status = Command::new("systemctl")
        .args(["disable", SERVICE_NAME])
        .status()?;
    if !status.success() {
        anyhow::bail!("failed to disable {}", SERVICE_NAME);
    }

    let timestamp = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    fs::write(MARKER_PATH, format!("completed_at={}\n", timestamp))?;
    Ok(())
}
