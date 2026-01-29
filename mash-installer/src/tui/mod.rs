//! TUI Module - Full Ratatui wizard for MASH Installer
//!
//! Provides an interactive terminal user interface with:
//! - Single-screen install flow
//! - Live progress dashboard

mod app;
mod input;
pub mod progress;
mod ui;
mod widgets;
mod new_app;
mod new_ui;

pub use app::{
    App, DownloadType, DownloadUpdate, FlashConfig, ImageSource, InputResult, InstallStep,
};

use crate::{cli::Cli, errors::Result};
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};
use std::io;
use std::time::Duration;
use std::thread;

/// Run the TUI wizard
pub fn run(cli: &Cli, watch: bool, dry_run: bool) -> Result<app::InputResult> {
    use std::io::IsTerminal;

    // Check if we have a real terminal
    if !std::io::stdout().is_terminal() {
        anyhow::bail!(
            "No TTY detected. The TUI requires an interactive terminal.\n\
             Try running directly in a terminal (not piped or via script).\n\
             If using sudo, try: sudo -E mash"
        );
    }

    // Set up terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Create app state
    let mut app = new_app::App::new();
    app.steps.push(new_app::InstallStep {
        name: "Partition Planning".to_string(),
        state: new_app::StepState::Pending,
        task: Box::new(|| Ok(())),
    });
    app.steps.push(new_app::InstallStep {
        name: "Download Fedora Image".to_string(),
        state: new_app::StepState::Pending,
        task: Box::new(|| Ok(())),
    });


    // Main loop
    let wizard_result = run_new_ui(&mut terminal, &mut app);

    // Restore terminal
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    // For now, we'll just return Quit
    Ok(app::InputResult::Quit)
}

/// Main application loop (single screen)
pub fn run_new_ui(terminal: &mut Terminal<CrosstermBackend<io::Stdout>>, app: &mut new_app::App) -> Result<()> {
    let tx = app.progress_tx.clone().unwrap();

    // Spawn a thread to simulate work
    thread::spawn(move || {
        for i in 0..app.steps.len() {
            tx.send(new_app::ProgressEvent {
                step_id: i,
                message: "Starting...".to_string(),
                progress: 0.0,
            }).unwrap();
            thread::sleep(Duration::from_secs(1));
            tx.send(new_app::ProgressEvent {
                step_id: i,
                message: "In progress...".to_string(),
                progress: 0.5,
            }).unwrap();
            thread::sleep(Duration::from_secs(1));
            tx.send(new_app::ProgressEvent {
                step_id: i,
                message: "Done.".to_string(),
                progress: 1.0,
            }).unwrap();
        }
    });


    loop {
        // Draw UI
        terminal.draw(|f| new_ui::draw(f, app))?;

        // Handle input with timeout
        if event::poll(Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                if key.modifiers.contains(KeyModifiers::CONTROL)
                    && (key.code == KeyCode::Char('c') || key.code == KeyCode::Char('q'))
                {
                    return Ok(());
                }
            }
        }

        // Check for progress updates
        if let Some(ref rx) = app.progress_rx {
            while let Ok(event) = rx.try_recv() {
                if event.progress == 0.0 {
                    app.steps[event.step_id].state = new_app::StepState::Running;
                }
                if event.progress == 1.0 {
                    app.steps[event.step_id].state = new_app::StepState::Completed;
                }
                app.status_message = event.message;
            }
        }
    }
}
