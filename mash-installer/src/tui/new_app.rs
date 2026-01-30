//! New application state machine for the single-screen TUI

#![allow(dead_code)]

use crate::cli::PartitionScheme;
use std::path::PathBuf;
use std::sync::mpsc::{Receiver, Sender};
use crate::tui::flash_config::{FlashConfig, ImageEditionOption, ImageSource, ImageVersionOption};
use crossterm::event::{KeyCode, KeyEvent}; // New import for KeyEvent

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
            InstallStepType::Options => Some(InstallStepType::Confirmation),
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
            InstallStepType::Confirmation => Some(InstallStepType::Options),
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
                | InstallStepType::Confirmation
        )
    }
}


// ============================================================================
// App
// ============================================================================

/// Application state
pub struct App {
    pub current_step_type: InstallStepType, // NEW
    pub partition_plan: Option<PartitionPlan>,
    pub resolved_layout: Option<ResolvedPartitionLayout>,
    pub cleanup: Cleanup,
    pub progress_rx: Option<Receiver<ProgressEvent>>,
    pub progress_tx: Option<Sender<ProgressEvent>>,
    pub is_running: bool,
    pub status_message: String,
    pub error_message: Option<String>,
}

impl App {
    pub fn new() -> Self {
        let (tx, rx) = std::sync::mpsc::channel();
        Self {
            current_step_type: InstallStepType::Welcome, // NEW
            partition_plan: None,
            resolved_layout: None,
            cleanup: Cleanup { tasks: Vec::new() },
            progress_rx: Some(rx),
            progress_tx: Some(tx),
            is_running: false,
            status_message: "Welcome to MASH!".to_string(),
            error_message: None,
        }
    }

    // New: handle input for step advancement
    pub fn handle_input(&mut self, key: KeyEvent) -> InputResult {
        match self.current_step_type {
            InstallStepType::Welcome => self.handle_welcome_input(key),
            InstallStepType::PartitionScheme => self.handle_partition_scheme_input(key), // Specific handler for the stalling step
            _ => InputResult::Continue, // Default: just continue if no specific handler
        }
    }

    fn handle_welcome_input(&mut self, key: KeyEvent) -> InputResult {
        match key.code {
            KeyCode::Enter => {
                self.current_step_type = InstallStepType::DiskSelection;
                InputResult::Continue
            },
            KeyCode::Esc | KeyCode::Char('q') => InputResult::Quit,
            _ => InputResult::Continue,
        }
    }

    // Placeholder for PartitionPlanning input handler
    // Renamed from handle_partition_planning_input to handle_partition_scheme_input
    fn handle_partition_scheme_input(&mut self, key: KeyEvent) -> InputResult {
        match key.code {
            KeyCode::Enter => {
                self.current_step_type = InstallStepType::PartitionLayout; // Advance to next logical step
                InputResult::Continue
            },
            KeyCode::Esc => {
                if let Some(prev) = self.current_step_type.prev() {
                    self.current_step_type = prev;
                }
                InputResult::Continue
            },
            _ => InputResult::Continue,
        }
    }
}
