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

pub use app::App;

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
pub fn run(cli: &Cli, watch: bool, dry_run: bool) -> Result<()> {
    // Set up terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Create app state
    let mut app = App::new(cli, watch, dry_run);

    // Main loop
    let result = run_app(&mut terminal, &mut app);

    // Restore terminal
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    // Handle result
    match result {
        Ok(true) => {
            // User completed the wizard, run the flash
            if let Some(config) = app.get_flash_config() {
                crate::flash::run_with_progress(
                    cli,
                    &config.image,
                    &config.disk,
                    &config.uefi_dir,
                    config.dry_run,
                    config.auto_unmount,
                    true, // yes_i_know - already confirmed in TUI
                    config.watch,
                    Some(config.locale.clone()),
                    config.early_ssh,
                    config.progress_tx,
                )?;
            }
            Ok(())
        }
        Ok(false) => {
            // User cancelled
            log::info!("Installation cancelled by user.");
            Ok(())
        }
        Err(e) => Err(e),
    }
}

/// Main application loop
fn run_app(terminal: &mut Terminal<CrosstermBackend<io::Stdout>>, app: &mut App) -> Result<bool> {
    loop {
        // Draw UI
        terminal.draw(|f| ui::draw(f, app))?;

        // Handle input with timeout for progress updates
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

        // Check for progress updates if in progress screen
        app.update_progress();
    }
}
