use anyhow::Context;
use crossterm::event::{self, Event, KeyCode};
use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use mash_hal::ProcessOps;
use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, List, ListItem, Paragraph, Wrap};
use ratatui::{backend::CrosstermBackend, Terminal};
use std::fs;
use std::io::{self, Stdout};
use std::time::{Duration, SystemTime};

use mash_core::dojo_catalogue::{
    parse_catalogue_toml, CategorySpec, DojoCatalogue, InstallSpec, SupportedDistro,
};

const MARKER_PATH: &str = "/var/lib/mash/dojo.completed";
const SERVICE_NAME: &str = "mash-dojo.service";

const DEFAULT_CATALOGUE_TOML: &str = include_str!("default_catalogue.toml");

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Screen {
    MainMenu,
    Catalogue,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Focus {
    Categories,
    Programs,
}

struct App {
    screen: Screen,
    focus: Focus,
    menu_items: Vec<&'static str>,
    menu_selected: usize,

    catalogue: DojoCatalogue,
    category_index: usize,
    program_index: usize,
    show_more_choices: bool,

    // Selected program IDs (Fedora-only selection for now).
    selected_programs: std::collections::HashSet<String>,

    // Modal confirmation for "Spicy" programs.
    confirm_spicy: Option<String>,

    status: String,
}

impl App {
    fn new() -> anyhow::Result<Self> {
        let catalogue = parse_catalogue_toml(DEFAULT_CATALOGUE_TOML)?;
        Ok(Self {
            screen: Screen::MainMenu,
            focus: Focus::Categories,
            menu_items: vec![
                "Program catalogue",
                "Show quick tips",
                "Continue to desktop",
            ],
            menu_selected: 0,
            catalogue,
            category_index: 0,
            program_index: 0,
            show_more_choices: false,
            selected_programs: std::collections::HashSet::new(),
            confirm_spicy: None,
            status: "Welcome to MASH Dojo.".to_string(),
        })
    }

    fn select_next(idx: &mut usize, len: usize) {
        if len == 0 {
            *idx = 0;
            return;
        }
        *idx = (*idx + 1) % len;
    }

    fn select_prev(idx: &mut usize, len: usize) {
        if len == 0 {
            *idx = 0;
            return;
        }
        if *idx == 0 {
            *idx = len - 1;
        } else {
            *idx -= 1;
        }
    }

    fn current_category(&self) -> Option<&CategorySpec> {
        self.catalogue.categories.get(self.category_index)
    }

    fn visible_programs(&self) -> Vec<&InstallSpec> {
        let Some(cat) = self.current_category() else {
            return Vec::new();
        };
        cat.visible_programs(SupportedDistro::Fedora, self.show_more_choices)
    }

    fn current_program(&self) -> Option<&InstallSpec> {
        let visible = self.visible_programs();
        visible.get(self.program_index).copied()
    }

    fn toggle_program(&mut self, spec: &InstallSpec) {
        let id = spec.id.clone();
        if self.selected_programs.contains(&id) {
            self.selected_programs.remove(&id);
            self.status = format!("Removed: {}", spec.label);
            return;
        }

        // Basic conflict enforcement at selection time.
        for conflict in &spec.conflicts_with {
            if self.selected_programs.contains(conflict) {
                self.status = format!(
                    "Cannot select {} (conflicts with already-selected {}).",
                    spec.label, conflict
                );
                return;
            }
        }

        self.selected_programs.insert(id);
        self.status = format!("Selected: {}", spec.label);
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
    let mut app = App::new()?;

    loop {
        terminal.draw(|f| draw(f, &app))?;

        if event::poll(Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                // Modal confirmation takes precedence.
                if let Some(pending_id) = app.confirm_spicy.clone() {
                    match key.code {
                        KeyCode::Char('y') | KeyCode::Char('Y') => {
                            if let Some(spec) =
                                find_spec_by_id(&app.catalogue, &pending_id).cloned()
                            {
                                app.toggle_program(&spec);
                            }
                            app.confirm_spicy = None;
                        }
                        KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => {
                            app.confirm_spicy = None;
                            app.status = "Cancelled spicy selection.".to_string();
                        }
                        _ => {}
                    }
                    continue;
                }

                match app.screen {
                    Screen::MainMenu => match key.code {
                        KeyCode::Up => {
                            App::select_prev(&mut app.menu_selected, app.menu_items.len())
                        }
                        KeyCode::Down => {
                            App::select_next(&mut app.menu_selected, app.menu_items.len())
                        }
                        KeyCode::Enter => match app.menu_selected {
                            0 => {
                                app.screen = Screen::Catalogue;
                                app.focus = Focus::Categories;
                                app.status = "Browse categories. Tab switches focus.".to_string();
                            }
                            1 => {
                                app.status =
                                    "Tip: Use the MASH installer TUI to re-run installs safely."
                                        .to_string();
                            }
                            2 => {
                                finalize_dojo()?;
                                return Ok(());
                            }
                            _ => {}
                        },
                        KeyCode::Char('q') => return Ok(()),
                        _ => {}
                    },
                    Screen::Catalogue => match key.code {
                        KeyCode::Char('q') | KeyCode::Esc => {
                            app.screen = Screen::MainMenu;
                            app.status = "Back to menu.".to_string();
                        }
                        KeyCode::Tab => {
                            app.focus = match app.focus {
                                Focus::Categories => Focus::Programs,
                                Focus::Programs => Focus::Categories,
                            };
                        }
                        KeyCode::Char('m') | KeyCode::Char('M') => {
                            app.show_more_choices = !app.show_more_choices;
                            app.program_index = 0;
                        }
                        KeyCode::Up => match app.focus {
                            Focus::Categories => {
                                App::select_prev(
                                    &mut app.category_index,
                                    app.catalogue.categories.len(),
                                );
                                app.program_index = 0;
                            }
                            Focus::Programs => {
                                let len = app.visible_programs().len();
                                App::select_prev(&mut app.program_index, len);
                            }
                        },
                        KeyCode::Down => match app.focus {
                            Focus::Categories => {
                                App::select_next(
                                    &mut app.category_index,
                                    app.catalogue.categories.len(),
                                );
                                app.program_index = 0;
                            }
                            Focus::Programs => {
                                let len = app.visible_programs().len();
                                App::select_next(&mut app.program_index, len);
                            }
                        },
                        KeyCode::Enter => {
                            if let Some(spec) = app.current_program().cloned() {
                                if spec.risk_level == mash_core::dojo_catalogue::RiskLevel::Spicy {
                                    app.confirm_spicy = Some(spec.id.clone());
                                } else {
                                    app.toggle_program(&spec);
                                }
                            }
                        }
                        _ => {}
                    },
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

    match app.screen {
        Screen::MainMenu => {
            let items = app
                .menu_items
                .iter()
                .enumerate()
                .map(|(idx, item)| {
                    let marker = if idx == app.menu_selected { "▶" } else { " " };
                    ListItem::new(format!("{} {}", marker, item))
                })
                .collect::<Vec<_>>();
            let list = List::new(items).block(Block::default().borders(Borders::ALL).title("Menu"));
            f.render_widget(list, chunks[1]);
        }
        Screen::Catalogue => {
            let body = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Percentage(35), Constraint::Percentage(65)].as_ref())
                .split(chunks[1]);

            let cat_items = app
                .catalogue
                .categories
                .iter()
                .enumerate()
                .map(|(idx, cat)| {
                    let marker = if idx == app.category_index {
                        "▶"
                    } else {
                        " "
                    };
                    ListItem::new(format!("{} {}", marker, cat.label))
                })
                .collect::<Vec<_>>();

            let cat_title = match app.focus {
                Focus::Categories => "Categories (tab: focus)",
                Focus::Programs => "Categories",
            };
            let cat_list =
                List::new(cat_items).block(Block::default().borders(Borders::ALL).title(cat_title));
            f.render_widget(cat_list, body[0]);

            let visible = app.visible_programs();
            let prog_items = visible
                .iter()
                .enumerate()
                .map(|(idx, spec)| {
                    let cursor = if idx == app.program_index && app.focus == Focus::Programs {
                        "▶"
                    } else {
                        " "
                    };
                    let selected = if app.selected_programs.contains(&spec.id) {
                        "[x]"
                    } else {
                        "[ ]"
                    };
                    let spicy = if spec.risk_level == mash_core::dojo_catalogue::RiskLevel::Spicy {
                        " (Spicy)"
                    } else {
                        ""
                    };
                    ListItem::new(format!("{} {} {}{}", cursor, selected, spec.label, spicy))
                })
                .collect::<Vec<_>>();

            let toggle = if app.show_more_choices {
                "Show more choices: ON (top 5)"
            } else {
                "Show more choices: OFF (defaults)"
            };
            let prog_title = format!("Programs [{}]  (m: toggle, enter: select)", toggle);
            let prog_list = List::new(prog_items)
                .block(Block::default().borders(Borders::ALL).title(prog_title));
            f.render_widget(prog_list, body[1]);

            if let Some(cat) = app.current_category() {
                if let Some(desc) = cat.description.as_ref() {
                    let footer = Paragraph::new(desc.as_str())
                        .block(Block::default().borders(Borders::ALL).title("Category"))
                        .wrap(Wrap { trim: true });
                    // Render as an overlay in the bottom-left of the body to avoid extra layout complexity.
                    let area = ratatui::layout::Rect {
                        x: body[0].x,
                        y: body[0].y + body[0].height.saturating_sub(6),
                        width: body[0].width,
                        height: 6,
                    };
                    f.render_widget(Clear, area);
                    f.render_widget(footer, area);
                }
            }
        }
    }

    let help = Paragraph::new(app.status.as_str())
        .block(Block::default().borders(Borders::ALL).title("Status"));
    f.render_widget(help, chunks[2]);

    if let Some(pending_id) = app.confirm_spicy.as_ref() {
        if let Some(spec) = find_spec_by_id(&app.catalogue, pending_id) {
            let area = centered_rect(70, 40, f.area());
            f.render_widget(Clear, area);
            let text = format!(
                "This selection is marked as Spicy.\n\n\
                 Program: {}\n\n\
                 Implications:\n\
                 - May change system configuration\n\
                 - May require a reboot\n\n\
                 Confirm selection? (y/n)",
                spec.label
            );
            let block = Paragraph::new(text)
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .title("Spicy Confirmation"),
                )
                .wrap(Wrap { trim: true });
            f.render_widget(block, area);
        }
    }
}

fn find_spec_by_id<'a>(catalogue: &'a DojoCatalogue, id: &str) -> Option<&'a InstallSpec> {
    for cat in &catalogue.categories {
        for spec in &cat.programs {
            if spec.id == id {
                return Some(spec);
            }
        }
    }
    None
}

fn centered_rect(
    percent_x: u16,
    percent_y: u16,
    r: ratatui::layout::Rect,
) -> ratatui::layout::Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints(
            [
                Constraint::Percentage((100 - percent_y) / 2),
                Constraint::Percentage(percent_y),
                Constraint::Percentage((100 - percent_y) / 2),
            ]
            .as_ref(),
        )
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints(
            [
                Constraint::Percentage((100 - percent_x) / 2),
                Constraint::Percentage(percent_x),
                Constraint::Percentage((100 - percent_x) / 2),
            ]
            .as_ref(),
        )
        .split(popup_layout[1])[1]
}

fn finalize_dojo() -> anyhow::Result<()> {
    let marker_dir = std::path::Path::new("/var/lib/mash");
    fs::create_dir_all(marker_dir)?;

    let hal = mash_hal::LinuxHal::new();
    hal.command_status(
        "systemctl",
        &["disable", SERVICE_NAME],
        std::time::Duration::from_secs(60),
    )
    .map_err(anyhow::Error::new)
    .with_context(|| format!("failed to disable {}", SERVICE_NAME))?;

    let timestamp = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    fs::write(MARKER_PATH, format!("completed_at={}\n", timestamp))?;
    Ok(())
}
