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
use std::sync::mpsc;
use std::thread;
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
                // Create progress channel
                let (tx, rx) = mpsc::channel();

                // Store receiver in app for progress updates
                // We need to run flash in a separate thread to keep TUI responsive

                // Re-enable terminal for progress display
                enable_raw_mode()?;
                let mut stdout = io::stdout();
                execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
                let backend = CrosstermBackend::new(stdout);
                let mut terminal = Terminal::new(backend)?;

                // Reset app to progress screen
                app.current_screen = app::Screen::Progress;
                app.progress = progress::ProgressState::default();
                app.progress_rx = Some(rx);

                // Spawn flash thread
                let image = config.image.clone();
                let disk = config.disk.clone();
                let uefi_dir = config.uefi_dir.clone();
                let scheme = config.scheme;
                let flash_dry_run = config.dry_run;
                let auto_unmount = config.auto_unmount;
                let locale = app.selected_locale().cloned();
                let early_ssh = config.early_ssh;
                let efi_size = config.efi_size.clone();
                let boot_size = config.boot_size.clone();
                let root_end = config.root_end.clone();

                let flash_handle = thread::spawn(move || {
                    crate::flash::run_with_progress(
                        &image,
                        &disk,
                        scheme,
                        &uefi_dir,
                        flash_dry_run,
                        auto_unmount,
                        true, // yes_i_know - already confirmed in TUI
                        locale,
                        early_ssh,
                        Some(tx),
                        &efi_size,
                        &boot_size,
                        &root_end,
                    )
                });

                // Run progress display loop
                let _ = run_progress_loop(&mut terminal, &mut app);

                // Wait for flash to complete
                let flash_result = flash_handle
                    .join()
                    .map_err(|_| anyhow::anyhow!("Flash thread panicked"))?;

                // Restore terminal
                disable_raw_mode()?;
                execute!(
                    terminal.backend_mut(),
                    LeaveAlternateScreen,
                    DisableMouseCapture
                )?;
                terminal.show_cursor()?;

                // Return flash result
                flash_result?;
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
fn run_progress_loop(
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
