//! TUI Module - Full Ratatui wizard for MASH Installer
//!
//! Provides an interactive terminal user interface with:
//! - Multi-screen wizard flow
//! - Disk and image selection
//! - Locale configuration
//! - Download progress screens
//! - Live progress dashboard
//! - SSH-friendly operation (no X11 required)

mod app;
mod input;
pub mod progress;
mod ui;
mod widgets;

pub use app::{App, DownloadPhase, DownloadUpdate, FlashConfig, ImageSource, Screen};

use crate::{cli::Cli, download, errors::Result};
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};
use std::io;
use std::path::PathBuf;
use std::thread;
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
    let mut download_thread: Option<thread::JoinHandle<anyhow::Result<PathBuf>>> = None;
    let mut last_screen = app.current_screen;

    loop {
        // Check for screen transitions that need download threads
        if app.current_screen != last_screen {
            // Screen changed - check if we need to start a download
            match app.current_screen {
                Screen::DownloadingFedora => {
                    // Start Fedora download thread
                    let tx = app.setup_download_channel();
                    let dest_dir = app.mash_root.join("downloads").join("images");
                    let version = app::ImageVersionOption::all()[app.selected_image_version_index]
                        .version_str()
                        .to_string();
                    let edition = app::ImageEditionOption::all()[app.selected_image_edition_index]
                        .edition_str()
                        .to_string();

                    download_thread = Some(thread::spawn(move || {
                        download::download_fedora_image_with_progress(
                            &dest_dir, &version, &edition, tx,
                        )
                    }));
                }
                Screen::DownloadingUefi => {
                    // Start UEFI download thread
                    let tx = app.setup_download_channel();
                    let dest_dir = app.mash_root.join("downloads").join("uefi");

                    download_thread = Some(thread::spawn(move || {
                        download::download_uefi_firmware_with_progress(&dest_dir, tx)
                    }));
                }
                _ => {}
            }
            last_screen = app.current_screen;
        }

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

        // Check for download updates
        app.update_download();

        // Check for progress updates if in progress screen
        app.update_progress();

        // Check if download thread completed and handle result
        if let Some(handle) = download_thread.take() {
            if handle.is_finished() {
                match handle.join() {
                    Ok(Ok(path)) => {
                        // Download succeeded - store the path
                        if app.current_screen == Screen::DownloadingFedora {
                            app.downloaded_image_path = Some(path);
                        } else if app.current_screen == Screen::DownloadingUefi {
                            app.downloaded_uefi_path = Some(path);
                        }
                        // Phase is already set to Complete by the download thread
                    }
                    Ok(Err(e)) => {
                        // Download failed
                        app.download_state.phase = DownloadPhase::Failed;
                        app.download_state.error = Some(e.to_string());
                    }
                    Err(_) => {
                        // Thread panicked
                        app.download_state.phase = DownloadPhase::Failed;
                        app.download_state.error = Some("Download thread crashed".to_string());
                    }
                }
            } else {
                // Thread still running, put it back
                download_thread = Some(handle);
            }
        }
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
