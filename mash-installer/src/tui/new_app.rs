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
// Stub UI Options (Phase B1)
// ============================================================================

#[derive(Debug, Clone)]
pub struct DiskOption {
    pub label: String,
    pub path: String,
    pub size: String,
}

#[derive(Debug, Clone)]
pub struct SourceOption {
    pub label: String,
    pub value: ImageSource,
}

#[derive(Debug, Clone)]
pub struct ImageOption {
    pub label: String,
    pub version: String,
    pub edition: String,
    pub path: PathBuf,
}

#[derive(Debug, Clone)]
pub struct OptionToggle {
    pub label: String,
    pub enabled: bool,
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
    pub fn all() -> &'static [InstallStepType] {
        &[
            InstallStepType::Welcome,
            InstallStepType::DiskSelection,
            InstallStepType::DiskConfirmation,
            InstallStepType::BackupConfirmation,
            InstallStepType::PartitionScheme,
            InstallStepType::PartitionLayout,
            InstallStepType::PartitionCustomize,
            InstallStepType::DownloadSourceSelection,
            InstallStepType::ImageSelection,
            InstallStepType::UefiDirectory,
            InstallStepType::LocaleSelection,
            InstallStepType::Options,
            InstallStepType::FirstBootUser,
            InstallStepType::Confirmation,
            InstallStepType::DownloadingFedora,
            InstallStepType::DownloadingUefi,
            InstallStepType::Flashing,
            InstallStepType::Complete,
        ]
    }

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
            InstallStepType::DownloadingUefi => None, // Execution steps do not have 'next' in this flow
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
    pub backup_choice_index: usize,
    pub disks: Vec<DiskOption>,
    pub disk_index: usize,
    pub disk_confirm_index: usize,
    pub partition_schemes: Vec<PartitionScheme>,
    pub scheme_index: usize,
    pub partition_layouts: Vec<String>,
    pub layout_index: usize,
    pub partition_customizations: Vec<String>,
    pub customize_index: usize,
    pub image_sources: Vec<SourceOption>,
    pub image_source_index: usize,
    pub images: Vec<ImageOption>,
    pub image_index: usize,
    pub uefi_dirs: Vec<PathBuf>,
    pub uefi_index: usize,
    pub locales: Vec<String>,
    pub locale_index: usize,
    pub options: Vec<OptionToggle>,
    pub options_index: usize,
    pub first_boot_options: Vec<String>,
    pub first_boot_index: usize,
    pub confirmation_index: usize,
    pub is_running: bool,
    pub status_message: String,
    pub error_message: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ListAction {
    Advance,
    Back,
    Quit,
    None,
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
            progress_rx: Some(rx),                 // Existing
            progress_tx: Some(tx),                 // Existing
            flash_progress_sender: Some(flash_tx), // NEW
            flash_progress_receiver: None,         // handled by background thread
            progress_state,
            backup_confirmed: false,
            backup_choice_index: 1,
            disks: vec![
                DiskOption {
                    label: "USB Disk 32GB".to_string(),
                    path: "/dev/sda".to_string(),
                    size: "32 GB".to_string(),
                },
                DiskOption {
                    label: "NVMe Disk 512GB".to_string(),
                    path: "/dev/nvme0n1".to_string(),
                    size: "512 GB".to_string(),
                },
            ],
            disk_index: 0,
            disk_confirm_index: 0,
            partition_schemes: vec![PartitionScheme::Mbr, PartitionScheme::Gpt],
            scheme_index: 0,
            partition_layouts: vec![
                "EFI 1024MiB | BOOT 2048MiB | ROOT 1800GiB | DATA rest".to_string(),
                "EFI 512MiB | BOOT 1024MiB | ROOT 64GiB | DATA rest".to_string(),
            ],
            layout_index: 0,
            partition_customizations: vec![
                "EFI 1024MiB".to_string(),
                "BOOT 2048MiB".to_string(),
                "ROOT 1800GiB".to_string(),
                "DATA remainder".to_string(),
            ],
            customize_index: 0,
            image_sources: vec![
                SourceOption {
                    label: "Local Image File (.raw)".to_string(),
                    value: ImageSource::LocalFile,
                },
                SourceOption {
                    label: "Download Fedora Image".to_string(),
                    value: ImageSource::DownloadFedora,
                },
            ],
            image_source_index: 0,
            images: vec![
                ImageOption {
                    label: "Fedora 43 KDE".to_string(),
                    version: "43".to_string(),
                    edition: "KDE".to_string(),
                    path: PathBuf::from("/tmp/fedora-43-kde.raw"),
                },
                ImageOption {
                    label: "Fedora 42 Server".to_string(),
                    version: "42".to_string(),
                    edition: "Server".to_string(),
                    path: PathBuf::from("/tmp/fedora-42-server.raw"),
                },
            ],
            image_index: 0,
            uefi_dirs: vec![
                PathBuf::from("/tmp/uefi"),
                PathBuf::from("/opt/uefi-firmware"),
            ],
            uefi_index: 0,
            locales: vec![
                "en_US.UTF-8:us".to_string(),
                "en_GB.UTF-8:uk".to_string(),
                "de_DE.UTF-8:de".to_string(),
            ],
            locale_index: 0,
            options: vec![
                OptionToggle {
                    label: "Auto-unmount target disk".to_string(),
                    enabled: true,
                },
                OptionToggle {
                    label: "Download UEFI firmware".to_string(),
                    enabled: false,
                },
                OptionToggle {
                    label: "Download Fedora image".to_string(),
                    enabled: false,
                },
                OptionToggle {
                    label: "Enable early SSH".to_string(),
                    enabled: false,
                },
            ],
            options_index: 0,
            first_boot_options: vec![
                "Prompt to create user on first boot (recommended)".to_string(),
                "Skip first-boot prompt".to_string(),
            ],
            first_boot_index: 0,
            confirmation_index: 0,
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
            InstallStepType::DiskSelection => {
                let len = self.disks.len();
                let action = Self::list_action(key, len, &mut self.disk_index);
                self.apply_list_action(action)
            }
            InstallStepType::DiskConfirmation => self.handle_disk_confirmation_input(key),
            InstallStepType::PartitionScheme => {
                let len = self.partition_schemes.len();
                let action = Self::list_action(key, len, &mut self.scheme_index);
                self.apply_list_action(action)
            }
            InstallStepType::PartitionLayout => {
                let len = self.partition_layouts.len();
                let action = Self::list_action(key, len, &mut self.layout_index);
                self.apply_list_action(action)
            }
            InstallStepType::PartitionCustomize => {
                let len = self.partition_customizations.len();
                let action = Self::list_action(key, len, &mut self.customize_index);
                self.apply_list_action(action)
            }
            InstallStepType::DownloadSourceSelection => {
                let len = self.image_sources.len();
                let action = Self::list_action(key, len, &mut self.image_source_index);
                self.apply_list_action(action)
            }
            InstallStepType::ImageSelection => {
                let len = self.images.len();
                let action = Self::list_action(key, len, &mut self.image_index);
                self.apply_list_action(action)
            }
            InstallStepType::UefiDirectory => {
                let len = self.uefi_dirs.len();
                let action = Self::list_action(key, len, &mut self.uefi_index);
                self.apply_list_action(action)
            }
            InstallStepType::LocaleSelection => {
                let len = self.locales.len();
                let action = Self::list_action(key, len, &mut self.locale_index);
                self.apply_list_action(action)
            }
            InstallStepType::Options => self.handle_options_input(key),
            InstallStepType::FirstBootUser => {
                let len = self.first_boot_options.len();
                let action = Self::list_action(key, len, &mut self.first_boot_index);
                self.apply_list_action(action)
            }
            InstallStepType::Confirmation => self.handle_confirmation_input(key),
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
            }
            KeyCode::Esc | KeyCode::Char('q') => InputResult::Quit,
            _ => InputResult::Continue,
        }
    }

    fn handle_generic_config_input(&mut self, key: KeyEvent) -> InputResult {
        match key.code {
            KeyCode::Enter => {
                self.error_message = None;
                if let Some(next) = self.current_step_type.next() {
                    self.current_step_type = next;
                }
                InputResult::Continue
            }
            KeyCode::Esc => {
                if let Some(prev) = self.current_step_type.prev() {
                    self.current_step_type = prev;
                }
                InputResult::Continue
            }
            KeyCode::Char('q') => InputResult::Quit,
            _ => InputResult::Continue,
        }
    }

    fn handle_backup_confirmation_input(&mut self, key: KeyEvent) -> InputResult {
        match key.code {
            KeyCode::Up | KeyCode::Down | KeyCode::Left | KeyCode::Right => {
                self.toggle_backup_choice();
                InputResult::Continue
            }
            KeyCode::Char('y') | KeyCode::Char('Y') => {
                self.backup_confirmed = true;
                self.backup_choice_index = 1;
                self.error_message = None;
                if let Some(next) = self.current_step_type.next() {
                    self.current_step_type = next;
                }
                InputResult::Continue
            }
            KeyCode::Char('n') | KeyCode::Char('N') => {
                self.backup_confirmed = false;
                self.backup_choice_index = 0;
                self.error_message = Some("Backup confirmation required to proceed.".to_string());
                if let Some(prev) = self.current_step_type.prev() {
                    self.current_step_type = prev;
                }
                InputResult::Continue
            }
            KeyCode::Enter => {
                if self.backup_choice_index == 1 {
                    self.backup_confirmed = true;
                    self.error_message = None;
                    if let Some(next) = self.current_step_type.next() {
                        self.current_step_type = next;
                    }
                } else {
                    self.backup_confirmed = false;
                    self.error_message =
                        Some("Backup confirmation required to proceed.".to_string());
                }
                InputResult::Continue
            }
            KeyCode::Esc => {
                if let Some(prev) = self.current_step_type.prev() {
                    self.current_step_type = prev;
                }
                InputResult::Continue
            }
            KeyCode::Char('q') => InputResult::Quit,
            _ => InputResult::Continue,
        }
    }

    fn handle_options_input(&mut self, key: KeyEvent) -> InputResult {
        match key.code {
            KeyCode::Up => {
                let len = self.options.len();
                Self::adjust_index(len, &mut self.options_index, -1);
                InputResult::Continue
            }
            KeyCode::Down => {
                let len = self.options.len();
                Self::adjust_index(len, &mut self.options_index, 1);
                InputResult::Continue
            }
            KeyCode::Char(' ') => {
                if let Some(option) = self.options.get_mut(self.options_index) {
                    option.enabled = !option.enabled;
                }
                InputResult::Continue
            }
            KeyCode::Enter => {
                self.error_message = None;
                if let Some(next) = self.current_step_type.next() {
                    self.current_step_type = next;
                }
                InputResult::Continue
            }
            KeyCode::Esc => {
                if let Some(prev) = self.current_step_type.prev() {
                    self.current_step_type = prev;
                }
                InputResult::Continue
            }
            KeyCode::Char('q') => InputResult::Quit,
            _ => InputResult::Continue,
        }
    }

    fn handle_disk_confirmation_input(&mut self, key: KeyEvent) -> InputResult {
        let options_len = 2;
        match key.code {
            KeyCode::Up | KeyCode::Left => {
                Self::adjust_index(options_len, &mut self.disk_confirm_index, -1);
                InputResult::Continue
            }
            KeyCode::Down | KeyCode::Right => {
                Self::adjust_index(options_len, &mut self.disk_confirm_index, 1);
                InputResult::Continue
            }
            KeyCode::Enter => {
                if self.disk_confirm_index == 0 {
                    self.error_message = None;
                    if let Some(next) = self.current_step_type.next() {
                        self.current_step_type = next;
                    }
                } else if let Some(prev) = self.current_step_type.prev() {
                    self.current_step_type = prev;
                }
                InputResult::Continue
            }
            KeyCode::Esc => {
                if let Some(prev) = self.current_step_type.prev() {
                    self.current_step_type = prev;
                }
                InputResult::Continue
            }
            KeyCode::Char('q') => InputResult::Quit,
            _ => InputResult::Continue,
        }
    }

    fn handle_confirmation_input(&mut self, key: KeyEvent) -> InputResult {
        match key.code {
            KeyCode::Enter => {
                if self.confirmation_index == 0 {
                    if !self.backup_confirmed {
                        self.error_message =
                            Some("Backup confirmation required before installation.".to_string());
                        return InputResult::Continue;
                    }
                    self.is_running = true;
                    self.current_step_type = InstallStepType::Flashing;
                    self.status_message = "🛠️ Starting installation...".to_string();
                    if let Some(config) = self.build_flash_config() {
                        return InputResult::StartFlash(config);
                    }
                    InputResult::Continue
                } else if let Some(prev) = self.current_step_type.prev() {
                    self.current_step_type = prev;
                    InputResult::Continue
                } else {
                    InputResult::Continue
                }
            }
            KeyCode::Up | KeyCode::Left => {
                Self::adjust_index(2, &mut self.confirmation_index, -1);
                InputResult::Continue
            }
            KeyCode::Down | KeyCode::Right => {
                Self::adjust_index(2, &mut self.confirmation_index, 1);
                InputResult::Continue
            }
            KeyCode::Esc => {
                if let Some(prev) = self.current_step_type.prev() {
                    self.current_step_type = prev;
                }
                InputResult::Continue
            }
            KeyCode::Char('q') => InputResult::Quit,
            _ => InputResult::Continue,
        }
    }

    fn apply_list_action(&mut self, action: ListAction) -> InputResult {
        match action {
            ListAction::Advance => {
                self.error_message = None;
                if let Some(next) = self.current_step_type.next() {
                    self.current_step_type = next;
                }
                InputResult::Continue
            }
            ListAction::Back => {
                if let Some(prev) = self.current_step_type.prev() {
                    self.current_step_type = prev;
                }
                InputResult::Continue
            }
            ListAction::Quit => InputResult::Quit,
            ListAction::None => InputResult::Continue,
        }
    }

    fn list_action(key: KeyEvent, len: usize, index: &mut usize) -> ListAction {
        match key.code {
            KeyCode::Up | KeyCode::Left => {
                Self::adjust_index(len, index, -1);
                ListAction::None
            }
            KeyCode::Down | KeyCode::Right => {
                Self::adjust_index(len, index, 1);
                ListAction::None
            }
            KeyCode::Enter => ListAction::Advance,
            KeyCode::Esc => ListAction::Back,
            KeyCode::Char('q') => ListAction::Quit,
            _ => ListAction::None,
        }
    }

    fn adjust_index(len: usize, index: &mut usize, delta: isize) {
        if len == 0 {
            *index = 0;
            return;
        }
        let len_i = len as isize;
        let mut next = *index as isize + delta;
        if next < 0 {
            next = len_i - 1;
        } else if next >= len_i {
            next = 0;
        }
        *index = next as usize;
    }

    fn toggle_backup_choice(&mut self) {
        self.backup_choice_index = if self.backup_choice_index == 0 { 1 } else { 0 };
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
            }
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
            image: self
                .images
                .get(self.image_index)
                .map(|image| image.path.clone())
                .unwrap_or_else(|| PathBuf::from("/tmp/placeholder_image.raw")),
            disk: self
                .disks
                .get(self.disk_index)
                .map(|disk| disk.path.clone())
                .unwrap_or_else(|| "/dev/sda".to_string()),
            scheme: *self
                .partition_schemes
                .get(self.scheme_index)
                .unwrap_or(&PartitionScheme::Mbr),
            uefi_dir: self
                .uefi_dirs
                .get(self.uefi_index)
                .cloned()
                .unwrap_or_else(|| PathBuf::from("/tmp/placeholder_uefi")),
            dry_run: false,
            auto_unmount: self
                .options
                .iter()
                .find(|option| option.label == "Auto-unmount target disk")
                .map(|option| option.enabled)
                .unwrap_or(true),
            watch: false,
            locale: None,
            early_ssh: self
                .options
                .iter()
                .find(|option| option.label == "Enable early SSH")
                .map(|option| option.enabled)
                .unwrap_or(false),
            progress_tx: self.flash_progress_sender.clone(), // This is the critical change!
            efi_size: "1024MiB".to_string(),
            boot_size: "2048MiB".to_string(),
            root_end: "1800GiB".to_string(),
            download_uefi_firmware: self
                .options
                .iter()
                .find(|option| option.label == "Download UEFI firmware")
                .map(|option| option.enabled)
                .unwrap_or(false),
            image_source_selection: self
                .image_sources
                .get(self.image_source_index)
                .map(|source| source.value)
                .unwrap_or(ImageSource::LocalFile),
            image_version: self
                .images
                .get(self.image_index)
                .map(|image| image.version.clone())
                .unwrap_or_else(|| "43".to_string()),
            image_edition: self
                .images
                .get(self.image_index)
                .map(|image| image.edition.clone())
                .unwrap_or_else(|| "KDE".to_string()),
        })
    }
}
