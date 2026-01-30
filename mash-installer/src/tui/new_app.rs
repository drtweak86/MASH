use super::new_ui::{
    self, centered_rect, draw, draw_cancel_dialog, draw_complete_panel, draw_config_panel,
    draw_disk_confirm_panel, draw_disk_selection_panel, draw_execution_panel,
    draw_final_summary_panel, draw_image_selection_panel, draw_image_source_panel,
    draw_locale_selection_panel, draw_options_panel, draw_partition_customize_panel,
    draw_partition_layout_panel, draw_partition_scheme_panel, draw_welcome_panel,
};
use super::progress::{self, ProgressEvent, ProgressUpdate};
use super::widgets::{DiskInfo, PartitionSize};
use crate::cli::Cli;
use crate::errors::Result;
use crate::tui::app::{self, InputResult, InstallStep};
use crate::tui::input::InputField;
use crate::tui::widgets::*;
use anyhow::anyhow;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use log::{error, info};
use ratatui::{backend::CrosstermBackend, Terminal};
use std::path::PathBuf;
use std::{
    collections::HashMap,
    io,
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc::{Receiver, Sender},
    },
    thread,
    time::Duration,
};

/// Main application state for the single-page TUI
pub struct App {
    /// Installation state (step states, progress, errors)
    pub state: InstallationState,
    /// UI state for interactive elements
    pub ui: UiState,
    /// Partition plan (scheme + sizes)
    pub partition_plan: PartitionPlan,
    /// Download state for current download (if any)
    pub download_state: DownloadState,
    /// Paths to downloaded files
    pub downloaded_paths: DownloadedPaths,
    /// Selected disk path
    pub selected_disk: Option<String>,
    /// Selected image path
    pub image_path: Option<PathBuf>,
    /// UEFI directory path
    pub uefi_dir: Option<PathBuf>,
    /// Selected locale
    pub locale: Option<LocaleConfig>,
    /// Image source selection
    pub image_source: ImageSource,
    /// Whether to download UEFI firmware
    pub download_uefi: bool,
    /// Dry-run mode
    pub dry_run: bool,
    /// Auto-unmount enabled
    pub auto_unmount: bool,
    /// Early SSH enabled
    pub early_ssh: bool,
    /// Progress event receiver
    pub progress_rx: Option<Receiver<ProgressEvent>>,
    /// Progress event sender (for passing to worker threads)
    pub progress_tx: Option<Sender<ProgressEvent>>,
    /// Cancellation flag (shared with worker threads)
    pub cancel_flag: Arc<AtomicBool>,
    /// Animation tick counter
    pub animation_tick: u64,
    /// Whether the app is running (not cancelled)
    pub is_running: bool,
    /// MASH root directory
    pub mash_root: PathBuf,
    /// Worker thread handle (if running)
    pub worker_handle: Option<thread::JoinHandle<WorkerResult>>,
    /// Guard that ensures cleanup runs exactly once
    pub cleanup_guard: CleanupGuard,
}

impl App {
    pub fn new() -> Self {
        let (tx, rx) = std::sync::mpsc::channel();
        let mut ui = UiState::default();

        // Scan for available disks
        ui.disks = DiskInfo::scan_disks();

        Self {
            state: InstallationState::default(),
            ui,
            partition_plan: PartitionPlan::default(),
            download_state: DownloadState::default(),
            downloaded_paths: DownloadedPaths::default(),
            selected_disk: None,
            image_path: None,
            uefi_dir: None,
            locale: None,
            image_source: ImageSource::LocalFile,
            download_uefi: false,
            dry_run: false,
            auto_unmount: true,
            early_ssh: true,
            progress_rx: Some(rx),
            progress_tx: Some(tx),
            cancel_flag: Arc::new(AtomicBool::new(false)),
            animation_tick: 0,
            is_running: true,
            mash_root: PathBuf::from("."),
            worker_handle: None,
            cleanup_guard: CleanupGuard::new(
                self.progress_tx
                    .clone()
                    .expect("Progress TX should be initialized before cleanup guard"),
            ),
        }
    }

    pub fn create_cleanup_guard(&self) -> CleanupGuard {
        CleanupGuard::new(self.progress_tx.clone())
    }

    pub fn request_cancel(&mut self) {
        info!("🛑 Cancellation requested");
        self.cancel_flag.store(true, Ordering::SeqCst);
        self.is_running = false;
        self.state.exec_state = ExecutionState::Cancelling;
        if let Some(ref tx) = self.progress_tx {
            let _ = tx.send(ProgressEvent::CancelRequested);
        }
    }

    pub fn is_cancelled(&self) -> bool {
        self.cancel_flag.load(Ordering::SeqCst)
    }

    pub fn get_cancel_flag(&self) -> Arc<AtomicBool> {
        self.cancel_flag.clone()
    }

    /// Check if worker thread is still running
    pub fn is_worker_running(&self) -> bool {
        self.worker_handle.is_some()
    }

    /// Check if worker has completed and collect result
    pub fn check_worker_completion(&mut self) -> Option<WorkerResult> {
        if let Some(handle) = self.worker_handle.take() {
            if handle.is_finished() {
                match handle.join() {
                    Ok(result) => {
                        info!("Worker completed with result: {:?}", result);
                        Some(result)
                    }
                    Err(e) => {
                        warn!("Worker thread panicked: {:?}", e);
                        Some(WorkerResult::Error("Worker thread panicked".to_string()))
                    }
                }
            } else {
                // Put it back, still running
                self.worker_handle = Some(handle);
                None
            }
        } else {
            None
        }
    }

    /// Set the worker handle (called when spawning worker)
    pub fn set_worker_handle(&mut self, handle: thread::JoinHandle<WorkerResult>) {
        self.worker_handle = Some(handle);
        self.state.set_running();
    }

    /// Build FlashConfig from current app state
    pub fn build_flash_config(&self) -> crate::tui::FlashConfig {
        crate::tui::FlashConfig {
            image: self.image_path.clone().unwrap_or_default(),
            disk: self.selected_disk.clone().unwrap_or_default(),
            scheme: self.partition_plan.scheme,
            uefi_dir: self.uefi_dir.clone().unwrap_or_default(),
            dry_run: self.dry_run,
            auto_unmount: self.auto_unmount,
            watch: false,
            locale: self.locale.clone(),
            early_ssh: self.early_ssh,
            progress_tx: None, // Will be set by start_execution
            cancel_flag: Arc::new(AtomicBool::new(false)), // Will be set by start_execution
            efi_size: self.partition_plan.efi_size.display(),
            boot_size: self.partition_plan.boot_size.display(),
            root_end: self.partition_plan.root_end.display(),
            download_uefi_firmware: self.download_uefi,
            image_source_selection: self.image_source,
            image_version: match self.ui.image_version_idx {
                0 => "43".to_string(),
                _ => "42".to_string(),
            },
            image_edition: "KDE".to_string(),
        }
    }

    /// Validate the current partition plan
    pub fn validate_partition_plan(&self) -> Result<(), String> {
        validate_partition_plan(&self.partition_plan)
    }

    /// Refresh disk list
    pub fn refresh_disks(&mut self) {
        self.ui.disks = DiskInfo::scan_disks();
        self.ui.selected_disk_idx = 0;
    }

    /// Get current config step (or first if none active)
    pub fn current_config_step(&self) -> ConfigStep {
        self.state
            .current_config
            .unwrap_or(ConfigStep::DiskSelection)
    }

    /// Check if we're in execution phase
    pub fn is_executing(&self) -> bool {
        self.state.mode == InstallMode::Executing
    }

    /// Check if we're in config phase
    pub fn is_configuring(&self) -> bool {
        self.state.mode == InstallMode::Configuring
    }

    /// Start configuration phase
    pub fn start_configuring(&mut self) {
        self.state.mode = InstallMode::Configuring;
        self.state.current_config = Some(ConfigStep::DiskSelection);
        self.state
            .config_states
            .insert(ConfigStep::DiskSelection, StepState::Current);
    }

    /// Move to next config step
    pub fn next_config_step(&mut self) -> bool {
        let current = self.current_config_step();

        // Mark current as completed
        self.state
            .config_states
            .insert(current, StepState::Completed);

        // Find next step
        let all_steps = ConfigStep::all();
        let current_idx = all_steps.iter().position(|s| *s == current).unwrap_or(0);

        if current_idx + 1 < all_steps.len() {
            let next = all_steps[current_idx + 1];
            self.state.current_config = Some(next);
            self.state.config_states.insert(next, StepState::Current);
            true
        } else {
            // Reached end of config steps
            self.state.current_config = None;
            false
        }
    }

    /// Move to previous config step
    pub fn prev_config_step(&mut self) -> bool {
        let current = self.current_config_step();

        // Mark current as pending (going back)
        self.state.config_states.insert(current, StepState::Pending);

        // Find previous step
        let all_steps = ConfigStep::all();
        let current_idx = all_steps.iter().position(|s| *s == current).unwrap_or(0);

        if current_idx > 0 {
            let prev = all_steps[current_idx - 1];
            self.state.current_config = Some(prev);
            self.state.config_states.insert(prev, StepState::Current);
            true
        } else {
            false
        }
    }

    /// Start execution phase and spawn worker thread
    pub fn start_execution(&mut self) {
        self.state.mode = InstallMode::Executing;
        self.state.current_config = None;
        self.state.status_message = "Starting installation...".to_string();
        self.ui
            .exec_log
            .push("Starting installation...".to_string());

        // Build flash config from current app state
        let mut flash_config = self.build_flash_config();

        // Clone handles needed by the worker
        let progress_tx = self
            .progress_tx
            .clone()
            .expect("Progress TX should be available when starting execution");
        let cancel_flag = self.cancel_flag.clone();

        // Insert progress_tx and cancel_flag into config for the worker
        flash_config.progress_tx = Some(progress_tx.clone());
        flash_config.cancel_flag = cancel_flag.clone();

        // Spawn worker thread
        let worker_handle = thread::spawn(move || {
            // The flash function will use the cloned progress_tx and cancel_flag via FlashContext
            // It may send ProgressEvent::FlashUpdate etc.
            // Any error is sent as ProgressEvent::Error
            if let Err(e) = flash::run_installation_pipeline(&flash_config, true) {
                let _ = progress_tx.send(ProgressEvent::Error(e.to_string()));
            }
        });

        self.worker_handle = Some(worker_handle);
        self.state.set_running();
    }

    /// Handle input for current step
    pub fn handle_key(&mut self, code: crossterm::event::KeyCode) -> InputAction {
        use crossterm::event::KeyCode;

        // Handle cancel dialog first
        if self.ui.cancel_dialog_visible {
            return match code {
                KeyCode::Char('y') | KeyCode::Char('Y') => {
                    self.ui.cancel_dialog_visible = false;
                    InputAction::ConfirmCancel
                }
                KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => {
                    self.ui.cancel_dialog_visible = false;
                    InputAction::DenyCancel
                }
                _ => InputAction::Continue,
            };
        }

        // Handle based on mode
        match self.state.mode {
            InstallMode::Welcome => self.handle_welcome_key(code),
            InstallMode::Configuring => self.handle_config_key(code),
            InstallMode::Executing => self.handle_executing_key(code),
            InstallMode::Complete => self.handle_complete_key(code),
        }
    }

    fn handle_welcome_key(&mut self, code: crossterm::event::KeyCode) -> InputAction {
        use crossterm::event::KeyCode;
        match code {
            KeyCode::Enter => {
                self.start_configuring();
                InputAction::Continue
            }
            KeyCode::Esc | KeyCode::Char('q') => InputAction::Exit,
            _ => InputAction::Continue,
        }
    }

    fn handle_config_key(&mut self, code: crossterm::event::KeyCode) -> InputAction {
        let step = self.current_config_step();
        match step {
            ConfigStep::DiskSelection => self.handle_disk_selection_key(code),
            ConfigStep::DiskConfirmation => self.handle_disk_confirm_key(code),
            ConfigStep::PartitionScheme => self.handle_partition_scheme_key(code),
            ConfigStep::PartitionLayout => self.handle_partition_layout_key(code),
            ConfigStep::PartitionCustomize => self.handle_partition_customize_key(code),
            ConfigStep::ImageSource => self.handle_image_source_key(code),
            ConfigStep::ImageSelection => self.handle_image_selection_key(code),
            ConfigStep::UefiSource => self.handle_uefi_source_key(code),
            ConfigStep::LocaleSelection => self.handle_locale_selection_key(code),
            ConfigStep::Options => self.handle_options_key(code),
            ConfigStep::FinalSummary => self.handle_final_summary_key(code),
        }
    }

    fn handle_executing_key(&mut self, code: crossterm::event::KeyCode) -> InputAction {
        use crossterm::event::KeyCode;
        match code {
            KeyCode::Esc | KeyCode::Char('q') => {
                self.ui.cancel_dialog_visible = true;
                InputAction::RequestCancel
            }
            _ => InputAction::Continue,
        }
    }

    fn handle_complete_key(&mut self, code: crossterm::event::KeyCode) -> InputAction {
        use crossterm::event::KeyCode;
        match code {
            KeyCode::Enter | KeyCode::Esc | KeyCode::Char('q') => InputAction::Exit,
            _ => InputAction::Continue,
        }
    }

    // Step-specific handlers (unchanged, omitted for brevity) ...
}
