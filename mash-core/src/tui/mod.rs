//! TUI Module - The MASH Dojo UI (Ratatui-based installer flow)
//!
//! Provides an interactive terminal user interface with:
//! - Single-screen install flow
//! - Live progress dashboard

mod data_sources;
mod dojo_app;
mod dojo_ui;
mod input;
pub mod progress;

mod widgets;

pub mod flash_config; // Declare the new module
pub use flash_config::{FlashConfig, ImageSource}; // Update the pub use statement

use crate::download::DownloadProgress;
use crate::download_manager;
use crate::tui::progress::{Phase, ProgressUpdate};
use crate::{cli::Cli, errors::Result, flash};
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};
use std::io;
use std::path::PathBuf;
use std::sync::mpsc;

use std::time::Duration;

/// Run the Dojo UI (interactive TUI).
pub fn run(_cli: &Cli, _watch: bool, _dry_run: bool) -> Result<dojo_app::InputResult> {
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
    let mut app = dojo_app::App::new_with_flags(_dry_run);

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
    let mut app = dojo_app::App::new();
    for step in dojo_app::InstallStepType::all() {
        app.current_step_type = *step;
        let dump = dojo_ui::dump_step(&app);
        println!("{}", dump);
    }
    Ok(())
}

/// Main application loop (single screen)
pub fn run_new_ui(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    app: &mut dojo_app::App,
) -> Result<dojo_app::InputResult> {
    let mut flash_result_rx: Option<mpsc::Receiver<Result<DownloadOutcome>>> = None;
    loop {
        // Draw UI
        terminal.draw(|f| dojo_ui::draw(f, app))?;

        // Handle input with timeout
        if event::poll(Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                if key.code == KeyCode::F(12) {
                    let dump = dojo_ui::dump_step(app);
                    println!("{}", dump);
                    continue;
                }
                // Pass key event to app's handler
                let input_result = app.handle_input(key);
                match input_result {
                    dojo_app::InputResult::Quit => return Ok(dojo_app::InputResult::Quit),
                    dojo_app::InputResult::Complete => return Ok(dojo_app::InputResult::Complete),
                    dojo_app::InputResult::StartFlash(config) => {
                        if app.is_running {
                            continue;
                        }
                        app.is_running = true;
                        app.status_message = "⬇️ Starting downloads...".to_string();
                        let (tx, rx) = mpsc::channel();
                        flash_result_rx = Some(rx);
                        let cancel_flag = app.cancel_requested.clone();
                        let yes_i_know = app.destructive_armed;
                        std::thread::spawn(move || {
                            let result = run_execution_pipeline(config, yes_i_know, cancel_flag);
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
                // This logic needs to be updated to map to the dojo_app::App's state more accurately.
                // For now, just update status message
                app.status_message = event.message;
            }
        }

        if app.current_step_type == dojo_app::InstallStepType::Flashing {
            let is_complete = app
                .progress_state
                .lock()
                .map(|state| state.is_complete)
                .unwrap_or(false);
            if is_complete {
                app.current_step_type = dojo_app::InstallStepType::Complete;
                app.status_message = "🎉 Dry-run complete.".to_string();
            }
        }

        if let Some(ref rx) = flash_result_rx {
            if let Ok(result) = rx.try_recv() {
                match result {
                    Ok(outcome) => {
                        if outcome.cancelled {
                            if let Ok(mut state) = app.progress_state.lock() {
                                state.apply_update(ProgressUpdate::Error("Cancelled".to_string()));
                            }
                            app.error_message = Some("Download cancelled.".to_string());
                            app.status_message = "🛑 Download cancelled.".to_string();
                        } else {
                            app.downloaded_image_path = outcome.image_path;
                            app.downloaded_uefi_dir = outcome.uefi_dir;
                            app.status_message = "✅ Downloads complete.".to_string();
                        }
                    }
                    Err(err) => {
                        let msg = err.to_string();
                        if msg.to_lowercase().contains("cancel") {
                            if let Ok(mut state) = app.progress_state.lock() {
                                state.apply_update(ProgressUpdate::Error("Cancelled".to_string()));
                            }
                            app.error_message = Some("Execution cancelled.".to_string());
                            app.status_message = "🛑 Execution cancelled.".to_string();
                        } else {
                            if let Ok(mut state) = app.progress_state.lock() {
                                state.apply_update(ProgressUpdate::Error(msg.clone()));
                            }
                            app.error_message = Some(format!("Execution failed: {}", msg));
                            app.status_message = "❌ Execution failed.".to_string();
                        }
                    }
                }
            }
        }
    }
}

struct DownloadOutcome {
    image_path: Option<PathBuf>,
    uefi_dir: Option<PathBuf>,
    cancelled: bool,
}

fn run_execution_pipeline(
    mut config: FlashConfig,
    yes_i_know: bool,
    cancel_flag: std::sync::Arc<std::sync::atomic::AtomicBool>,
) -> Result<DownloadOutcome> {
    flash::set_cancel_flag(cancel_flag.clone());
    let tx = config.progress_tx.clone();
    let send = |update: ProgressUpdate| {
        if let Some(ref tx) = tx {
            let _ = tx.send(update);
        }
    };

    let mut downloaded_image = None;
    let mut downloaded_uefi = None;
    let download_root = PathBuf::from("/tmp/mash-downloads");

    if config.image_source_selection == ImageSource::DownloadFedora {
        send(ProgressUpdate::PhaseStarted(Phase::DownloadImage));
        send(ProgressUpdate::Status(
            "⬇️ Downloading Fedora image...".to_string(),
        ));
        let mut stage = |msg: &str| {
            send(ProgressUpdate::Status(msg.to_string()));
        };
        let mut progress = |progress: DownloadProgress| {
            if let Some(total) = progress.total {
                let percent = (progress.downloaded as f64 / total as f64) * 100.0;
                let speed_mbps = progress.speed_bytes_per_sec as f64 / (1024.0 * 1024.0);
                send(ProgressUpdate::RsyncProgress {
                    percent,
                    speed_mbps,
                    files_done: 0,
                    files_total: 0,
                });
            }
            !cancel_flag.load(std::sync::atomic::Ordering::Relaxed)
        };
        match download_manager::fetch_fedora_image(
            &download_root,
            &config.image_version,
            &config.image_edition,
            &mut progress,
            &mut stage,
            Some(cancel_flag.as_ref()),
        ) {
            Ok(path) => {
                downloaded_image = Some(path);
                send(ProgressUpdate::PhaseCompleted(Phase::DownloadImage));
            }
            Err(err) => {
                send(ProgressUpdate::Status("🧹 Cleaning up...".to_string()));
                if err.to_string().to_lowercase().contains("cancel") {
                    send(ProgressUpdate::Error("Cancelled".to_string()));
                    return Ok(DownloadOutcome {
                        image_path: None,
                        uefi_dir: None,
                        cancelled: true,
                    });
                }
                send(ProgressUpdate::Error(err.to_string()));
                return Err(err);
            }
        }
    } else {
        send(ProgressUpdate::PhaseSkipped(Phase::DownloadImage));
    }

    if config.download_uefi_firmware {
        send(ProgressUpdate::PhaseStarted(Phase::DownloadUefi));
        send(ProgressUpdate::Status(
            "⬇️ Downloading UEFI bundle...".to_string(),
        ));
        let mut stage = |msg: &str| {
            send(ProgressUpdate::Status(msg.to_string()));
        };
        let mut progress = |progress: DownloadProgress| {
            if let Some(total) = progress.total {
                let percent = (progress.downloaded as f64 / total as f64) * 100.0;
                let speed_mbps = progress.speed_bytes_per_sec as f64 / (1024.0 * 1024.0);
                send(ProgressUpdate::RsyncProgress {
                    percent,
                    speed_mbps,
                    files_done: 0,
                    files_total: 0,
                });
            }
            !cancel_flag.load(std::sync::atomic::Ordering::Relaxed)
        };
        match download_manager::fetch_uefi_bundle(
            &download_root,
            &mut progress,
            &mut stage,
            Some(cancel_flag.as_ref()),
        ) {
            Ok(path) => {
                downloaded_uefi = Some(path);
                send(ProgressUpdate::PhaseCompleted(Phase::DownloadUefi));
            }
            Err(err) => {
                send(ProgressUpdate::Status("🧹 Cleaning up...".to_string()));
                if err.to_string().to_lowercase().contains("cancel") {
                    send(ProgressUpdate::Error("Cancelled".to_string()));
                    return Ok(DownloadOutcome {
                        image_path: downloaded_image,
                        uefi_dir: None,
                        cancelled: true,
                    });
                }
                send(ProgressUpdate::Error(err.to_string()));
                return Err(err);
            }
        }
    } else {
        send(ProgressUpdate::PhaseSkipped(Phase::DownloadUefi));
    }

    if cancel_flag.load(std::sync::atomic::Ordering::Relaxed) {
        send(ProgressUpdate::Status("🧹 Cleaning up...".to_string()));
        if let Some(ref path) = downloaded_image {
            let _ = std::fs::remove_file(path);
        }
        if let Some(ref path) = downloaded_uefi {
            let _ = std::fs::remove_dir_all(path);
        }
        send(ProgressUpdate::Error("Cancelled".to_string()));
        flash::clear_cancel_flag();
        return Ok(DownloadOutcome {
            image_path: None,
            uefi_dir: None,
            cancelled: true,
        });
    }

    if let Some(path) = downloaded_image.clone() {
        config.image = path;
    }
    if let Some(path) = downloaded_uefi.clone() {
        config.uefi_dir = path;
    }

    let flash_result = flash::run_with_progress(&config, yes_i_know);
    flash::clear_cancel_flag();

    match flash_result {
        Ok(()) => Ok(DownloadOutcome {
            image_path: downloaded_image,
            uefi_dir: downloaded_uefi,
            cancelled: false,
        }),
        Err(err) => Err(err),
    }
}
