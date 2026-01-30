//! TUI Module - Full Ratatui wizard for MASH Installer
//!
//! Provides an interactive terminal user interface with:
//! - Single-screen install flow
//! - Live progress dashboard

mod app;
mod input;
mod new_app;
mod new_ui;
pub mod progress;

mod widgets;

pub mod flash_config; // Declare the new module
pub use flash_config::{FlashConfig, ImageSource}; // Update the pub use statement

use crate::{cli::Cli, errors::Result, flash};
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};
use std::io;
use std::sync::mpsc;

use std::time::Duration;

/// Run the TUI wizard
pub fn run(_cli: &Cli, _watch: bool, _dry_run: bool) -> Result<new_app::InputResult> {
    // Changed app::InputResult to new_app::InputResult
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

    // Main loop
    let final_result = run_new_ui(&mut terminal, &mut app)?;

    // Restore terminal
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    Ok(final_result)
}

pub fn dump_all_steps() -> Result<()> {
    let mut app = new_app::App::new();
    for step in new_app::InstallStepType::all() {
        app.current_step_type = *step;
        let dump = new_ui::dump_step(&app);
        println!("{}", dump);
    }
    Ok(())
}

/// Main application loop (single screen)
pub fn run_new_ui(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    app: &mut new_app::App,
) -> Result<new_app::InputResult> {
    // Return InputResult for handling in run()
    let mut flash_result_rx: Option<mpsc::Receiver<Result<()>>> = None;
    loop {
        // Draw UI
        terminal.draw(|f| new_ui::draw(f, app))?;

        // Handle input with timeout
        if event::poll(Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                if key.code == KeyCode::F(12) {
                    let dump = new_ui::dump_step(app);
                    println!("{}", dump);
                    continue;
                }
                // Pass key event to app's handler
                let input_result = app.handle_input(key);
                match input_result {
                    new_app::InputResult::Quit => return Ok(new_app::InputResult::Quit),
                    new_app::InputResult::Complete => return Ok(new_app::InputResult::Complete),
                    new_app::InputResult::StartFlash(config) => {
                        if app.is_running {
                            continue;
                        }
                        app.is_running = true;
                        app.status_message = "🛠️ Flashing started...".to_string();
                        let (tx, rx) = mpsc::channel();
                        flash_result_rx = Some(rx);
                        let yes_i_know = app.backup_confirmed;
                        std::thread::spawn(move || {
                            let result = flash::run_with_progress(&config, yes_i_know);
                            let _ = tx.send(result);
                        });
                    }
                    _ => {} // Continue, StartDownload are handled by app internally for now
                }
            }
        }

        // Check for progress updates (still needed for asynchronous updates)
        if let Some(ref rx) = app.progress_rx {
            while let Ok(event) = rx.try_recv() {
                // This logic needs to be updated to map to the new_app::App's state more accurately
                // For now, just update status message
                app.status_message = event.message;
            }
        }

        if let Some(ref rx) = flash_result_rx {
            if let Ok(result) = rx.try_recv() {
                match result {
                    Ok(()) => {
                        app.status_message = "🎉 Flashing finished.".to_string();
                    }
                    Err(err) => {
                        app.error_message = Some(format!("Flash failed: {}", err));
                        app.status_message = "❌ Flashing failed.".to_string();
                    }
                }
            }
        }
    }
}
