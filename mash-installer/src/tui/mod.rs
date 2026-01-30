//! TUI Module - Full Ratatui wizard for MASH Installer
//!
//! Provides an interactive terminal user interface with:
//! - Multi-screen wizard flow
//! - Disk and image selection
//! - Locale configuration
//! - Live progress dashboard
//! - SSH-friendly operation (no X11 required)

mod app;
mod input;
pub mod progress;
mod ui;
mod widgets;

pub use app::{App, FlashConfig, ImageSource, Screen};

use crate::{cli::Cli, errors::Result};
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};
use std::io;
use std::time::Duration;

/// Run the TUI wizard
pub fn run(cli: &Cli, watch: bool, dry_run: bool) -> Result<Option<app::FlashConfig>> {
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
    let mut app = App::new(cli, watch, dry_run);

    // Main loop
    let wizard_result = run_app(&mut terminal, &mut app);

    // Restore terminal
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    // Handle result
    match wizard_result {
        Ok(true) => {
            // User completed the wizard, return the config
            Ok(app.get_flash_config())
        }
        Ok(false) => {
            // User cancelled
            log::info!("Installation cancelled by user.");
            Ok(None)
        }
        Err(e) => Err(e),
    }
}

/// Main application loop (wizard screens)
fn run_app(terminal: &mut Terminal<CrosstermBackend<io::Stdout>>, app: &mut App) -> Result<bool> {
    loop {
        // Draw UI
        terminal.draw(|f| ui::draw(f, app))?;

        // Handle input with timeout for animations
        if event::poll(Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                // Global quit: Ctrl+C or Ctrl+Q
                if key.modifiers.contains(KeyModifiers::CONTROL)
                    && (key.code == KeyCode::Char('c') || key.code == KeyCode::Char('q'))
                {
                    return Ok(false);
                }

                // Handle screen-specific input
                match app.handle_input(key) {
                    app::InputResult::Continue => {}
                    app::InputResult::Quit => return Ok(false),
                    app::InputResult::Complete => return Ok(true),
                }
            }
        }

        // Increment animation tick for spinners and effects
        app.animation_tick = app.animation_tick.wrapping_add(1);

        // Check for progress updates if in progress screen
        app.update_progress();
    }
}

/// Progress display loop (runs during flash)
pub fn run_progress_loop(
    // Make public
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    app: &mut App,
) -> Result<()> {
    loop {
        // Draw UI
        terminal.draw(|f| ui::draw(f, app))?;

        // Handle input with timeout
        if event::poll(Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                // Global quit: Ctrl+C
                if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
                    // TODO: Signal flash to abort
                    log::warn!("Installation abort requested");
                    return Ok(());
                }

                // If complete, allow exit
                if app.progress.is_complete
                    && (key.code == KeyCode::Enter
                        || key.code == KeyCode::Esc
                        || key.code == KeyCode::Char('q'))
                {
                    return Ok(());
                }
            }
        }

        // Increment animation tick
        app.animation_tick = app.animation_tick.wrapping_add(1);

        // Check for progress updates
        app.update_progress();

        // Exit when complete and user has been shown the result
        if app.progress.is_complete && app.current_screen == app::Screen::Complete {
            // Wait for user input to exit (handled above)
        }

        // Auto-transition to complete screen when installation finishes
        if app.progress.is_complete && app.current_screen == app::Screen::Progress {
            app.current_screen = app::Screen::Complete;
            app.install_success = app.progress.error.is_none();
            app.install_error = app.progress.error.clone();
        }
    }
}
