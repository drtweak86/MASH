use app::*;
pub mod new_app;
pub mod new_ui;
pub mod progress;
pub mod ui;
pub mod widgets;

/// Run the TUI wizard – now the **new single‑page UI** is the only entry point.
/// The old multi‑screen (`run_legacy`) entry point has been removed.
pub fn run(cli: &Cli, watch: bool, dry_run: bool) -> Result<app::InputResult> {
    use std::io::IsTerminal;

    // Terminal sanity check
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

    // Create the **new** app state (single‑page UI)
    let mut app = new_app::App::new();
    app.dry_run = dry_run;

    // Populate mash_root paths from CLI
    let mash_root = &cli.mash_root;
    app.uefi_dir = Some(mash_root.join("uefi"));
    app.image_path = Some(mash_root.join("images"));

    // Create cleanup guard that will run on any exit path
    let cleanup_guard = app.create_cleanup_guard();

    // Main loop with cleanup guard
    let result = run_new_ui_loop(&mut terminal, &mut app, cleanup_guard);

    // Restore terminal (always, even on error)
    let _ = disable_raw_mode();
    let _ = execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    );
    let _ = terminal.show_cursor();

    // Return result
    match result {
        Ok(_) => Ok(app::InputResult::Quit),
        Err(e) => {
            // Log the error but don't propagate – we want a clean exit
            log::error!("TUI error: {}", e);
            Ok(app::InputResult::Quit)
        }
    }
}

/// Main application loop (single screen) with cleanup guard
///
/// The cleanup_guard ensures resources are cleaned up on:
/// - Normal exit (Esc, q, Ctrl+C)
/// - Error/panic
/// - Any early return via ?
fn run_new_ui_loop(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    app: &mut new_app::App,
    mut cleanup_guard: new_app::CleanupGuard,
) -> Result<()> {
    info!("🎬 Starting TUI event loop");

    // Note: In the full implementation, the cleanup_guard would be passed to
    // worker threads that register resources as they create them.
    // For now, we just hold it to ensure cleanup on exit.

    loop {
        // Tick animation counter
        app.tick();

        // Process any pending progress events (non‑blocking)
        app.process_events();

        // Draw UI
        terminal.draw(|f| new_ui::draw(f, app))?;

        // Handle input with timeout (non‑blocking, allows animation)
        match handle_input_tick(app)? {
            LoopAction::Continue => {}
            LoopAction::Exit => {
                info!("🛑 Exit requested, running cleanup...");
                // Explicitly run cleanup (will also run on drop, but this is clearer)
                let warnings = cleanup_guard.run_cleanup();
                if !warnings.is_empty() {
                    app.state.cleanup_warnings = warnings;
                }
                break;
            }
        }

        // Check if cancelled by worker thread
        if app.is_cancelled() && !cleanup_guard.is_cleaned_up() {
            info!("🛑 Cancellation detected, running cleanup...");
            let warnings = cleanup_guard.run_cleanup();
            if !warnings.is_empty() {
                app.state.cleanup_warnings = warnings;
            }
            break;
        }
    }

    info!("👋 TUI event loop ended");
    Ok(())
}

/// Handle a single input tick (non‑blocking)
fn handle_input_tick(app: &mut new_app::App) -> Result<LoopAction> {
    // Poll with timeout to allow animation updates (~10 FPS)
    if event::poll(Duration::from_millis(100))? {
        if let Event::Key(key) = event::read()? {
            // Global Ctrl+C/Ctrl+Q exit (always works, even in dialogs)
            if key.modifiers.contains(KeyModifiers::CONTROL) {
                match key.code {
                    KeyCode::Char('c') | KeyCode::Char('q') => {
                        app.request_cancel();
                        return Ok(LoopAction::Exit);
                    }
                    _ => {}
                }
            }

            // Delegate to app's step‑specific input handling
            match app.handle_key(key.code) {
                new_app::InputAction::Continue => {}
                new_app::InputAction::NextStep => {
                    // Step navigation handled internally by app
                }
                new_app::InputAction::PrevStep => {
                    // Step navigation handled internally by app
                }
                new_app::InputAction::StartExecution => {
                    // TODO: Pass #4 will spawn worker thread for installation
                    app.start_execution();
                    info!("🚀 Starting execution phase");
                }
                new_app::InputAction::RequestCancel => {
                    // Cancel dialog is now visible, handled by app.handle_key
                }
                new_app::InputAction::ConfirmCancel => {
                    app.request_cancel();
                    return Ok(LoopAction::Exit);
                }
                new_app::InputAction::Exit => {
                    app.request_cancel();
                    return Ok(LoopAction::Exit);
                }
            }
        }
    }

    Ok(LoopAction::Continue)
}

// ============================================================================
// Legacy run function (kept for reference, may be removed later)
// ============================================================================

/// Run the legacy multi‑screen TUI wizard
///
/// This is the original implementation with separate screens per step.
/// Kept for reference during transition to single‑page UI.
#[allow(dead_code)]
pub fn run_legacy(cli: &Cli, watch: bool, dry_run: bool) -> Result<app::InputResult> {
    use std::io::IsTerminal;

    if !std::io::stdout().is_terminal() {
        anyhow::bail!(
            "No TTY detected. The TUI requires an interactive terminal.\n\
             Try running directly in a terminal (not piped or via script).\n\
             If using sudo, try: sudo -E mash"
        );
    }

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Create legacy app state
    let mut legacy_app = app::App::new(cli, watch, dry_run);

    // Main loop using legacy UI
    let wizard_result = run_legacy_loop(&mut terminal, &mut legacy_app);

    // Restore terminal
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    wizard_result
}

/// Legacy event loop
#[allow(dead_code)]
fn run_legacy_loop(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    app: &mut app::App,
) -> Result<app::InputResult> {
    loop {
        // Update animation
        app.animation_tick = app.animation_tick.wrapping_add(1);

        // Update progress from channels
        app.update_progress();
        let download_result = app.update_download();

        // Handle download completion triggering next step
        if let app::InputResult::StartFlash(_) | app::InputResult::StartDownload(_) = download_result {
            return Ok(download_result);
        }

        // Draw UI
        terminal.draw(|f| ui::draw(f, app))?;

        // Handle input
        if event::poll(Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                // Global Ctrl+C handling
                if key.modifiers.contains(KeyModifiers::CONTROL)
                    && (key.code == KeyCode::Char('c') || key.code == KeyCode::Char('q'))
                {
                    return Ok(app::InputResult::Quit);
                }

                match app.handle_input(key) {
                    app::InputResult::Continue => {}
                    other => return Ok(other),
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_app_creation() {
        let app = new_app::App::new();
        assert!(!app.is_cancelled());
        assert!(app.is_running);
        assert_eq!(app.state.overall_percent, 0.0);
    }
}
