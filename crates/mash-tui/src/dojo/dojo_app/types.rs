use super::super::data_sources;
use super::super::flash_config::ImageSource;
use mash_core::cli::PartitionScheme;
use std::path::PathBuf;

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
    pub identity: data_sources::DiskIdentity,
    pub stable_id: Option<String>,
    pub path: String,
    /// Canonical label (HAL/sysfs) used for UI display.
    pub label: String,
    pub removable: bool,
    pub boot_confidence: data_sources::BootConfidence,
    pub is_source_disk: bool,
}

#[derive(Debug, Clone)]
pub struct SourceOption {
    pub label: String,
    pub value: ImageSource,
}

#[derive(Debug, Clone)]
pub struct OsVariantOption {
    pub id: String,    // Variant id from docs/os-download-links.toml
    pub label: String, // Human display label
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CustomizeField {
    Efi,
    Boot,
    Root,
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
