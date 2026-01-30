//! New application state machine for the single-screen TUI

#![allow(dead_code)]

use crate::cli::PartitionScheme;
use crate::tui::flash_config::{FlashConfig, ImageSource};
use crossterm::event::{KeyCode, KeyEvent}; // New import for KeyEvent
use std::path::PathBuf;
use std::sync::mpsc::{Receiver, Sender};
use std::sync::{Arc, Mutex};

// ============================================================================
// Step State
// ============================================================================

/// Represents the state of an installation step
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StepState {
    Pending,
    Running,
    Completed,
    Failed,
    Skipped,
}

// ============================================================================
// Install Step (old struct, to be eventually removed)
// ============================================================================

/// Represents an installation step (old struct, to be eventually removed)
pub struct InstallStep {
    pub name: String,
    pub state: StepState,
    pub task: Box<dyn Fn() -> Result<(), String> + Send>,
}

// ============================================================================
// Partition Plan
// ============================================================================

/// Represents the user's partition plan
#[derive(Debug, Clone)]
pub struct PartitionPlan {
    pub scheme: PartitionScheme,
    pub partitions: Vec<Partition>,
}

/// Represents a partition in the plan
#[derive(Debug, Clone)]
pub struct Partition {
    pub name: String,
    pub size: String, // e.g., "1024M", "2G", "100%"
    pub format: String,
    pub flags: Vec<String>,
}

// ============================================================================
// Resolved Layout
// ============================================================================

/// Represents the resolved partition layout
#[derive(Debug, Clone)]
pub struct ResolvedPartitionLayout {
    pub scheme: PartitionScheme,
    pub partitions: Vec<ResolvedPartition>,
}

/// Represents a resolved partition with a specific size in bytes
#[derive(Debug, Clone)]
pub struct ResolvedPartition {
    pub name: String,
    pub size_bytes: u64,
    pub format: String,
    pub flags: Vec<String>,
}

// ============================================================================
// Progress Event
// ============================================================================

/// Represents a progress event
#[derive(Debug, Clone)]
pub struct ProgressEvent {
    pub step_id: usize, // The index of the step in the `InstallStep` vector
    pub message: String,
    pub progress: f32, // A value between 0.0 and 1.0
}

// ============================================================================
// Cleanup
// ============================================================================

/// The cleanup guard
pub struct Cleanup {
    // A list of cleanup tasks to perform
    pub tasks: Vec<Box<dyn Fn() + Send>>,
}

impl Drop for Cleanup {
    fn drop(&mut self) {
        for task in &self.tasks {
            task();
        }
    }
}



// ============================================================================
// Result of handling input
// ============================================================================

/// Result of handling input
pub enum InputResult {
    Continue,
    Quit,
    Complete,
    StartFlash(FlashConfig),
    StartDownload(DownloadType),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DownloadType {
    FedoraImage {
        version: String,
        edition: String,
        dest_dir: PathBuf,
    },
    UefiFirmware {
        dest_dir: PathBuf,
    },
}

// ============================================================================
// TUI Install Steps
// ============================================================================

/// Defines the sequence of steps in the TUI wizard
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstallStepType {
    Welcome,
    DiskSelection,
    DiskConfirmation,
    BackupConfirmation, // New step for backup
    PartitionScheme,
    PartitionLayout,
    PartitionCustomize,
    DownloadSourceSelection,
    ImageSelection,
    UefiDirectory,
    LocaleSelection,
    Options,
    FirstBootUser,
    Confirmation,
    DownloadingFedora,
    DownloadingUefi,
    Flashing,
    Complete,
}

impl InstallStepType {
    pub fn title(&self) -> &'static str {
        match self {
            InstallStepType::Welcome => "Enter the Dojo",
            InstallStepType::DiskSelection => "Select Target Disk",
            InstallStepType::DiskConfirmation => "Confirm Disk Destruction",
            InstallStepType::BackupConfirmation => "Backup Confirmation", // Title for new step
            InstallStepType::PartitionScheme => "Partition Scheme",
            InstallStepType::PartitionLayout => "Partition Layout",
            InstallStepType::PartitionCustomize => "Customize Partitions",
            InstallStepType::DownloadSourceSelection => "Select Image Source",
            InstallStepType::ImageSelection => "Select Image File",
            InstallStepType::UefiDirectory => "UEFI Configuration",
            InstallStepType::LocaleSelection => "Locale & Keymap",
            InstallStepType::Options => "Installation Options",
            InstallStepType::FirstBootUser => "First-Boot User",
            InstallStepType::Confirmation => "Final Confirmation",
            InstallStepType::DownloadingFedora => "Downloading Fedora Image",
            InstallStepType::DownloadingUefi => "Downloading UEFI Firmware",
            InstallStepType::Flashing => "Installing...",
            InstallStepType::Complete => "Installation Complete!",
        }
    }

    // Helper to get the next step in the sequence
    pub fn next(&self) -> Option<InstallStepType> {
        match self {
            InstallStepType::Welcome => Some(InstallStepType::DiskSelection),
            InstallStepType::DiskSelection => Some(InstallStepType::DiskConfirmation),
            InstallStepType::DiskConfirmation => Some(InstallStepType::BackupConfirmation), // Insert new step
            InstallStepType::BackupConfirmation => Some(InstallStepType::PartitionScheme), // After backup, go to PartitionScheme
            InstallStepType::PartitionScheme => Some(InstallStepType::PartitionLayout),
            InstallStepType::PartitionLayout => Some(InstallStepType::DownloadSourceSelection),
            InstallStepType::PartitionCustomize => Some(InstallStepType::DownloadSourceSelection), // Customize also goes to DownloadSourceSelection
            InstallStepType::DownloadSourceSelection => Some(InstallStepType::ImageSelection),
            InstallStepType::ImageSelection => Some(InstallStepType::UefiDirectory),
            InstallStepType::UefiDirectory => Some(InstallStepType::LocaleSelection),
            InstallStepType::LocaleSelection => Some(InstallStepType::Options),
            InstallStepType::Options => Some(InstallStepType::FirstBootUser),
            InstallStepType::FirstBootUser => Some(InstallStepType::Confirmation),
            InstallStepType::Confirmation => None,
            InstallStepType::DownloadingFedora => None, // Execution steps do not have 'next' in this flow
            InstallStepType::DownloadingUefi => None,   // Execution steps do not have 'next' in this flow
            InstallStepType::Flashing => Some(InstallStepType::Complete),
            InstallStepType::Complete => None,
        }
    }

    // Helper to get the previous step in the sequence
    pub fn prev(&self) -> Option<InstallStepType> {
        match self {
            InstallStepType::Welcome => None,
            InstallStepType::DiskSelection => Some(InstallStepType::Welcome),
            InstallStepType::DiskConfirmation => Some(InstallStepType::DiskSelection),
            InstallStepType::BackupConfirmation => Some(InstallStepType::DiskConfirmation), // Previous to BackupConfirmation
            InstallStepType::PartitionScheme => Some(InstallStepType::BackupConfirmation), // Previous to PartitionScheme
            InstallStepType::PartitionLayout => Some(InstallStepType::PartitionScheme),
            InstallStepType::PartitionCustomize => Some(InstallStepType::PartitionLayout),
            InstallStepType::DownloadSourceSelection => Some(InstallStepType::PartitionLayout), // Customize also goes to DownloadSourceSelection
            InstallStepType::ImageSelection => Some(InstallStepType::DownloadSourceSelection),
            InstallStepType::UefiDirectory => Some(InstallStepType::ImageSelection),
            InstallStepType::LocaleSelection => Some(InstallStepType::UefiDirectory),
            InstallStepType::Options => Some(InstallStepType::LocaleSelection),
            InstallStepType::FirstBootUser => Some(InstallStepType::Options),
            InstallStepType::Confirmation => Some(InstallStepType::FirstBootUser),
            _ => None, // Execution steps do not have 'prev' in this flow
        }
    }

    // Check if this step is part of the configuration phase
    pub fn is_config_step(&self) -> bool {
        matches!(
            self,
            InstallStepType::Welcome
                | InstallStepType::DiskSelection
                | InstallStepType::DiskConfirmation
                | InstallStepType::BackupConfirmation
                | InstallStepType::PartitionScheme
                | InstallStepType::PartitionLayout
                | InstallStepType::PartitionCustomize
                | InstallStepType::DownloadSourceSelection
                | InstallStepType::ImageSelection
                | InstallStepType::UefiDirectory
                | InstallStepType::LocaleSelection
                | InstallStepType::Options
                | InstallStepType::FirstBootUser
                | InstallStepType::Confirmation
        )
    }
}


// ============================================================================
// App
// ============================================================================

use super::progress::ProgressState; // New import

// ...

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UserCreationField {
    Username,
    Password,
    PasswordConfirm,
}

#[derive(Debug, Clone)]
pub struct UserCreation {
    pub username: String,
    pub password: String,
    pub password_confirm: String,
    pub active_field: UserCreationField,
    pub is_complete: bool,
}

impl Default for UserCreation {
    fn default() -> Self {
        Self {
            username: String::new(),
            password: String::new(),
            password_confirm: String::new(),
            active_field: UserCreationField::Username,
            is_complete: false,
        }
    }
}

/// Application state
pub struct App {
    pub current_step_type: InstallStepType, // NEW
    pub partition_plan: Option<PartitionPlan>,
    pub resolved_layout: Option<ResolvedPartitionLayout>,
    pub cleanup: Cleanup,
    pub progress_rx: Option<Receiver<ProgressEvent>>, // Existing
    pub progress_tx: Option<Sender<ProgressEvent>>,   // Existing
    pub flash_progress_sender: Option<Sender<super::progress::ProgressUpdate>>, // NEW
    pub flash_progress_receiver: Option<Receiver<super::progress::ProgressUpdate>>, // NEW
    pub progress_state: Arc<Mutex<ProgressState>>,
    pub backup_confirmed: bool,
    pub user_creation: UserCreation,
    pub is_running: bool,
    pub status_message: String,
    pub error_message: Option<String>,
}

impl App {
    pub fn new() -> Self {
        let (tx, rx) = std::sync::mpsc::channel(); // Existing for ProgressEvent
        let (flash_tx, flash_rx) = std::sync::mpsc::channel(); // NEW for ProgressUpdate
        let progress_state = Arc::new(Mutex::new(ProgressState::default()));
        let progress_state_thread = Arc::clone(&progress_state);
        std::thread::spawn(move || {
            while let Ok(update) = flash_rx.recv() {
                if let Ok(mut state) = progress_state_thread.lock() {
                    state.apply_update(update);
                }
            }
        });
        Self {
            current_step_type: InstallStepType::Welcome, // NEW
            partition_plan: None,
            resolved_layout: None,
            cleanup: Cleanup { tasks: Vec::new() },
            progress_rx: Some(rx), // Existing
            progress_tx: Some(tx), // Existing
            flash_progress_sender: Some(flash_tx), // NEW
            flash_progress_receiver: None, // handled by background thread
            progress_state,
            backup_confirmed: false,
            user_creation: UserCreation::default(),
            is_running: false,
            status_message: "👋 Welcome to MASH!".to_string(),
            error_message: None,
        }
    }

    // New: handle input for step advancement
    pub fn handle_input(&mut self, key: KeyEvent) -> InputResult {
        match self.current_step_type {
            InstallStepType::Welcome => self.handle_welcome_input(key),
            InstallStepType::BackupConfirmation => self.handle_backup_confirmation_input(key),
            InstallStepType::FirstBootUser => self.handle_first_boot_user_input(key),
            step if step.is_config_step() => self.handle_generic_config_input(key),
            InstallStepType::Flashing => self.handle_flashing_input(key),
            _ => InputResult::Continue, // Default: just continue if no specific handler
        }
    }

    fn handle_welcome_input(&mut self, key: KeyEvent) -> InputResult {
        match key.code {
            KeyCode::Enter => {
                self.current_step_type = InstallStepType::DiskSelection;
                self.error_message = None;
                InputResult::Continue
            },
            KeyCode::Esc | KeyCode::Char('q') => InputResult::Quit,
            _ => InputResult::Continue,
        }
    }

    fn handle_generic_config_input(&mut self, key: KeyEvent) -> InputResult {
        match key.code {
            KeyCode::Enter => {
                if self.current_step_type == InstallStepType::Confirmation {
                    if !self.backup_confirmed {
                        self.error_message =
                            Some("Backup confirmation required before installation.".to_string());
                        return InputResult::Continue;
                    }
                    if !self.user_creation.is_complete {
                        self.error_message =
                            Some("First-boot user must be created before installation.".to_string());
                        return InputResult::Continue;
                    }
                    self.is_running = true;
                    self.current_step_type = InstallStepType::Flashing;
                    self.status_message = "🛠️ Starting installation...".to_string();
                    if let Some(config) = self.build_flash_config() {
                        return InputResult::StartFlash(config);
                    }
                }
                self.error_message = None;
                if let Some(next) = self.current_step_type.next() {
                    self.current_step_type = next;
                }
                InputResult::Continue
            },
            KeyCode::Esc => {
                if let Some(prev) = self.current_step_type.prev() {
                    self.current_step_type = prev;
                }
                InputResult::Continue
            },
            KeyCode::Char('q') => InputResult::Quit,
            _ => InputResult::Continue,
        }
    }

    fn handle_backup_confirmation_input(&mut self, key: KeyEvent) -> InputResult {
        match key.code {
            KeyCode::Char('y') | KeyCode::Char('Y') => {
                self.backup_confirmed = true;
                self.error_message = None;
                if let Some(next) = self.current_step_type.next() {
                    self.current_step_type = next;
                }
                InputResult::Continue
            },
            KeyCode::Char('n') | KeyCode::Char('N') => {
                self.backup_confirmed = false;
                self.error_message = Some("Backup confirmation required to proceed.".to_string());
                if let Some(prev) = self.current_step_type.prev() {
                    self.current_step_type = prev;
                }
                InputResult::Continue
            },
            KeyCode::Esc => {
                if let Some(prev) = self.current_step_type.prev() {
                    self.current_step_type = prev;
                }
                InputResult::Continue
            },
            KeyCode::Char('q') => InputResult::Quit,
            _ => InputResult::Continue,
        }
    }

    fn handle_first_boot_user_input(&mut self, key: KeyEvent) -> InputResult {
        match key.code {
            KeyCode::Tab => {
                self.user_creation.active_field = match self.user_creation.active_field {
                    UserCreationField::Username => UserCreationField::Password,
                    UserCreationField::Password => UserCreationField::PasswordConfirm,
                    UserCreationField::PasswordConfirm => UserCreationField::Username,
                };
                self.error_message = None;
                InputResult::Continue
            },
            KeyCode::Backspace => {
                let target = match self.user_creation.active_field {
                    UserCreationField::Username => &mut self.user_creation.username,
                    UserCreationField::Password => &mut self.user_creation.password,
                    UserCreationField::PasswordConfirm => &mut self.user_creation.password_confirm,
                };
                target.pop();
                self.error_message = None;
                InputResult::Continue
            },
            KeyCode::Enter => {
                if self.user_creation.username.trim().is_empty() {
                    self.error_message = Some("Username is required.".to_string());
                    return InputResult::Continue;
                }
                if self.user_creation.password.is_empty() {
                    self.error_message = Some("Password is required.".to_string());
                    return InputResult::Continue;
                }
                if self.user_creation.password != self.user_creation.password_confirm {
                    self.error_message = Some("Passwords do not match.".to_string());
                    return InputResult::Continue;
                }
                self.user_creation.is_complete = true;
                self.error_message = None;
                if let Some(next) = self.current_step_type.next() {
                    self.current_step_type = next;
                }
                InputResult::Continue
            },
            KeyCode::Esc => {
                if let Some(prev) = self.current_step_type.prev() {
                    self.current_step_type = prev;
                }
                InputResult::Continue
            },
            KeyCode::Char('q') => InputResult::Quit,
            KeyCode::Char(c) => {
                let target = match self.user_creation.active_field {
                    UserCreationField::Username => &mut self.user_creation.username,
                    UserCreationField::Password => &mut self.user_creation.password,
                    UserCreationField::PasswordConfirm => &mut self.user_creation.password_confirm,
                };
                target.push(c);
                self.error_message = None;
                InputResult::Continue
            },
            _ => InputResult::Continue,
        }
    }

    fn handle_flashing_input(&mut self, key: KeyEvent) -> InputResult {
        let is_complete = self
            .progress_state
            .lock()
            .map(|state| state.is_complete)
            .unwrap_or(false);
        match key.code {
            KeyCode::Enter if is_complete => {
                self.current_step_type = InstallStepType::Complete;
                InputResult::Complete
            },
            KeyCode::Char('q') => InputResult::Quit,
            _ => InputResult::Continue,
        }
    }

    pub fn progress_state_snapshot(&self) -> ProgressState {
        self.progress_state
            .lock()
            .map(|state| state.clone())
            .unwrap_or_default()
    }

    /// Build flash configuration from current app state
    pub fn build_flash_config(&self) -> Option<FlashConfig> {
        // These values will eventually come from user selections in the TUI.
        // For now, we use placeholders to get it compiling.
        // The actual values would come from fields in the App struct (e.g., self.selected_disk, etc.)
        // This is a minimal implementation to allow FlashConfig construction.

        Some(FlashConfig {
            image: PathBuf::from("/tmp/placeholder_image.raw"),
            disk: "/dev/sda".to_string(), // Placeholder
            scheme: PartitionScheme::Mbr, // Placeholder
            uefi_dir: PathBuf::from("/tmp/placeholder_uefi"),
            dry_run: false, // Placeholder
            auto_unmount: true, // Placeholder
            watch: false, // Placeholder
            locale: None, // Placeholder
            early_ssh: false, // Placeholder
            progress_tx: self.flash_progress_sender.clone(), // This is the critical change!
            efi_size: "1024MiB".to_string(), // Placeholder
            boot_size: "2048MiB".to_string(), // Placeholder
            root_end: "1800GiB".to_string(), // Placeholder
            download_uefi_firmware: false, // Placeholder
            image_source_selection: ImageSource::LocalFile, // Placeholder
            image_version: "43".to_string(), // Placeholder
            image_edition: "KDE".to_string(), // Placeholder
        })
    }
}
