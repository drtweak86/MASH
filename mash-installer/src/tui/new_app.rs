//! New application state machine for the single-screen TUI

#![allow(dead_code)]

use crate::cli::PartitionScheme;
use std::path::PathBuf;
use std::sync::mpsc::{Receiver, Sender};
use crate::tui::flash_config::{FlashConfig, ImageEditionOption, ImageSource, ImageVersionOption};

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
// Install Step
// ============================================================================

/// Represents an installation step
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
// App
// ============================================================================

/// Application state
pub struct App {
    pub steps: Vec<InstallStep>,
    pub current_step: usize,
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
            steps: Vec::new(),
            current_step: 0,
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
}
