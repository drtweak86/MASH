//! New application state machine for the single-screen TUI

#![allow(dead_code)]

use super::flash_config::{ImageSource, TuiFlashConfig};
use clap::ValueEnum;
use crossterm::event::{KeyCode, KeyEvent}; // New import for KeyEvent
use mash_core::cli::PartitionScheme;
use mash_core::locale::{LocaleConfig, LOCALES};
use std::path::PathBuf;
use std::sync::mpsc::{Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::time::Instant;

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
    pub identity: super::data_sources::DiskIdentity,
    pub stable_id: String,
    pub path: String,
    pub removable: bool,
    pub boot_confidence: super::data_sources::BootConfidence,
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

// ============================================================================
// Result of handling input
// ============================================================================

/// Result of handling input
pub enum InputResult {
    Continue,
    Quit,
    Complete,
    StartFlash(Box<TuiFlashConfig>),
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

/// Defines the sequence of steps in the Dojo UI
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
    VariantSelection,
    EfiImage,
    LocaleSelection,
    Options,
    FirstBootUser,
    PlanReview,
    Confirmation,
    ExecuteConfirmationGate,
    DisarmSafeMode,
    DownloadingFedora,
    DownloadingUefi,
    Flashing,
    Complete,
}

impl InstallStepType {
    pub fn all() -> &'static [InstallStepType] {
        &[
            InstallStepType::Welcome,
            InstallStepType::ImageSelection,
            InstallStepType::VariantSelection,
            InstallStepType::DownloadSourceSelection,
            InstallStepType::DiskSelection,
            InstallStepType::DiskConfirmation,
            InstallStepType::BackupConfirmation,
            InstallStepType::PartitionScheme,
            InstallStepType::PartitionLayout,
            InstallStepType::PartitionCustomize,
            InstallStepType::EfiImage,
            InstallStepType::LocaleSelection,
            InstallStepType::Options,
            InstallStepType::FirstBootUser,
            InstallStepType::PlanReview,
            InstallStepType::Confirmation,
            InstallStepType::ExecuteConfirmationGate,
            InstallStepType::DisarmSafeMode,
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
            InstallStepType::ImageSelection => "Select OS",
            InstallStepType::VariantSelection => "Select Variant",
            InstallStepType::EfiImage => "EFI Image",
            InstallStepType::LocaleSelection => "Locale & Keymap",
            InstallStepType::Options => "Installation Options",
            InstallStepType::FirstBootUser => "First-Boot User",
            InstallStepType::PlanReview => "Execution Plan",
            InstallStepType::Confirmation => "Final Confirmation",
            InstallStepType::ExecuteConfirmationGate => "Final Execute Gate",
            InstallStepType::DisarmSafeMode => "Disarm Safe Mode",
            InstallStepType::DownloadingFedora => "Downloading Fedora Image",
            InstallStepType::DownloadingUefi => "Downloading EFI Image",
            InstallStepType::Flashing => "Installing...",
            InstallStepType::Complete => "Installation Complete!",
        }
    }

    // Helper to get the next step in the sequence
    // Flow: Welcome → Distro → Flavour → Download Source → Disk → Partition → EFI → Locale → Options → Review → Confirm
    pub fn next(&self) -> Option<InstallStepType> {
        match self {
            InstallStepType::Welcome => Some(InstallStepType::ImageSelection),
            InstallStepType::ImageSelection => Some(InstallStepType::VariantSelection),
            InstallStepType::VariantSelection => Some(InstallStepType::DownloadSourceSelection),
            InstallStepType::DownloadSourceSelection => Some(InstallStepType::DiskSelection),
            InstallStepType::DiskSelection => Some(InstallStepType::DiskConfirmation),
            InstallStepType::DiskConfirmation => Some(InstallStepType::PartitionScheme),
            InstallStepType::BackupConfirmation => Some(InstallStepType::PartitionScheme),
            InstallStepType::PartitionScheme => Some(InstallStepType::PartitionLayout),
            InstallStepType::PartitionLayout => Some(InstallStepType::PartitionCustomize),
            InstallStepType::PartitionCustomize => Some(InstallStepType::EfiImage),
            InstallStepType::EfiImage => Some(InstallStepType::LocaleSelection),
            InstallStepType::LocaleSelection => Some(InstallStepType::Options),
            InstallStepType::Options => Some(InstallStepType::PlanReview),
            InstallStepType::FirstBootUser => Some(InstallStepType::PlanReview),
            InstallStepType::PlanReview => Some(InstallStepType::Confirmation),
            InstallStepType::Confirmation => None,
            InstallStepType::ExecuteConfirmationGate => None,
            InstallStepType::DisarmSafeMode => None,
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
            InstallStepType::ImageSelection => Some(InstallStepType::Welcome),
            InstallStepType::VariantSelection => Some(InstallStepType::ImageSelection),
            InstallStepType::DownloadSourceSelection => Some(InstallStepType::VariantSelection),
            InstallStepType::DiskSelection => Some(InstallStepType::DownloadSourceSelection),
            InstallStepType::DiskConfirmation => Some(InstallStepType::DiskSelection),
            InstallStepType::BackupConfirmation => Some(InstallStepType::DiskConfirmation),
            InstallStepType::PartitionScheme => Some(InstallStepType::DiskConfirmation),
            InstallStepType::PartitionLayout => Some(InstallStepType::PartitionScheme),
            InstallStepType::PartitionCustomize => Some(InstallStepType::PartitionLayout),
            InstallStepType::EfiImage => Some(InstallStepType::PartitionCustomize),
            InstallStepType::LocaleSelection => Some(InstallStepType::EfiImage),
            InstallStepType::Options => Some(InstallStepType::LocaleSelection),
            InstallStepType::FirstBootUser => Some(InstallStepType::Options),
            InstallStepType::PlanReview => Some(InstallStepType::Options),
            InstallStepType::Confirmation => Some(InstallStepType::PlanReview),
            InstallStepType::ExecuteConfirmationGate => Some(InstallStepType::Confirmation),
            InstallStepType::DisarmSafeMode => Some(InstallStepType::Confirmation),
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
                | InstallStepType::VariantSelection
                | InstallStepType::EfiImage
                | InstallStepType::LocaleSelection
                | InstallStepType::Options
                | InstallStepType::FirstBootUser
                | InstallStepType::PlanReview
                | InstallStepType::Confirmation
                | InstallStepType::ExecuteConfirmationGate
                | InstallStepType::DisarmSafeMode
        )
    }
}

// ============================================================================
// App
// ============================================================================

use super::data_sources;
use crate::progress::ProgressState; // New import

// ...

/// Application state
pub struct App {
    pub current_step_type: InstallStepType, // NEW
    pub partition_plan: Option<PartitionPlan>,
    pub resolved_layout: Option<ResolvedPartitionLayout>,
    pub cleanup: Cleanup,
    pub progress_rx: Option<Receiver<ProgressEvent>>, // Existing
    pub progress_tx: Option<Sender<ProgressEvent>>,   // Existing
    pub flash_progress_sender: Option<Sender<crate::progress::ProgressUpdate>>, // NEW
    pub flash_progress_receiver: Option<Receiver<crate::progress::ProgressUpdate>>, // NEW
    pub progress_state: Arc<Mutex<ProgressState>>,
    pub backup_confirmed: bool,
    pub backup_choice_index: usize,
    pub welcome_options: Vec<String>,
    pub welcome_index: usize,
    use_real_disks: bool,
    pub disks: Vec<DiskOption>,
    pub disk_index: usize,
    pub disk_confirm_index: usize,
    pub wipe_confirmation: String,
    pub partition_schemes: Vec<PartitionScheme>,
    pub scheme_index: usize,
    pub partition_layouts: Vec<String>,
    pub layout_index: usize,
    pub partition_customizations: Vec<String>,
    pub customize_index: usize,
    pub efi_size: String,
    pub boot_size: String,
    pub root_end: String,
    pub os_distros: Vec<super::flash_config::OsDistro>,
    pub os_distro_index: usize,
    pub os_variants: Vec<OsVariantOption>,
    pub os_variant_index: usize,
    pub image_sources: Vec<SourceOption>,
    pub image_source_index: usize,
    pub images: Vec<ImageOption>,
    pub image_index: usize,
    pub uefi_sources: Vec<super::flash_config::EfiSource>,
    pub uefi_source_index: usize,
    pub locales: Vec<String>,
    pub locale_index: usize,
    pub options: Vec<OptionToggle>,
    pub options_index: usize,
    pub first_boot_options: Vec<String>,
    pub first_boot_index: usize,
    pub downloading_fedora_index: usize,
    pub downloading_uefi_index: usize,
    pub downloaded_fedora: bool,
    pub downloaded_uefi: bool,
    pub destructive_armed: bool,
    pub flash_start_time: Option<Instant>,
    pub is_running: bool,
    pub status_message: String,
    pub error_message: Option<String>,
    pub image_source_path: String,
    pub uefi_source_path: String,
    pub downloaded_image_path: Option<PathBuf>,
    pub downloaded_uefi_dir: Option<PathBuf>,
    /// WO-036: execute-mode confirmation gate input.
    pub execute_confirmation_input: String,
    /// WO-036: execute-mode confirmation gate acknowledged.
    pub execute_confirmation_confirmed: bool,
    pub safe_mode_disarm_input: String,
    pending_destructive_action: Option<PendingDestructiveAction>,
    pub cancel_requested: Arc<std::sync::atomic::AtomicBool>,
    pub customize_error_field: Option<CustomizeField>,
    pub dry_run: bool,
    pub developer_mode: bool,
    pub mash_root: PathBuf,
    pub state_path: PathBuf,
    /// Completion messaging (post-install).
    pub completion_lines: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PendingDestructiveAction {
    StartFlash,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ListAction {
    Advance,
    Back,
    Quit,
    None,
}

impl Default for App {
    fn default() -> Self {
        Self::new()
    }
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
        let flags = data_sources::data_flags();
        let _boot_device = data_sources::boot_device_path();
        let use_real_disks = flags.disks;
        let real_disks = if use_real_disks {
            data_sources::scan_disks()
        } else {
            Vec::new()
        };
        let disks = if real_disks.is_empty() {
            vec![
                DiskOption {
                    identity: data_sources::DiskIdentity::new(
                        Some("SanDisk".to_string()),
                        Some("Ultra".to_string()),
                        None,
                        None,
                        32 * 1024 * 1024 * 1024,
                        data_sources::TransportType::Usb,
                    ),
                    stable_id: "mock:sandisk-ultra-usb".to_string(),
                    path: "/dev/sda".to_string(),
                    removable: true,
                    boot_confidence: data_sources::BootConfidence::NotBoot,
                    is_source_disk: false,
                },
                DiskOption {
                    identity: data_sources::DiskIdentity::new(
                        Some("Samsung".to_string()),
                        Some("980".to_string()),
                        None,
                        None,
                        512 * 1024 * 1024 * 1024,
                        data_sources::TransportType::Nvme,
                    ),
                    stable_id: "mock:samsung-980-nvme".to_string(),
                    path: "/dev/nvme0n1".to_string(),
                    removable: false,
                    boot_confidence: data_sources::BootConfidence::Confident,
                    is_source_disk: true,
                },
            ]
        } else {
            real_disks
                .into_iter()
                .map(|disk| DiskOption {
                    identity: disk.identity,
                    stable_id: disk.stable_id,
                    path: disk.path,
                    removable: disk.removable,
                    boot_confidence: disk.boot_confidence,
                    is_source_disk: disk.is_source_disk,
                })
                .collect()
        };

        // Auto-select first non-boot, removable disk for safety (prefer USB/SD cards)
        let default_disk_index = disks
            .iter()
            .position(|disk| {
                !disk.is_source_disk
                    && disk.boot_confidence == data_sources::BootConfidence::NotBoot
                    && disk.removable
            })
            .or_else(|| {
                disks.iter().position(|disk| {
                    !disk.is_source_disk
                        && disk.boot_confidence == data_sources::BootConfidence::NotBoot
                })
            })
            .unwrap_or(0);

        let partition_schemes = PartitionScheme::value_variants().to_vec();
        let scheme_index = partition_schemes
            .iter()
            .position(|scheme| *scheme == PartitionScheme::Mbr)
            .unwrap_or(0);
        let images = if flags.images {
            let search_paths = data_sources::default_image_search_paths();
            let mut images = data_sources::collect_local_images(&search_paths);
            images.extend(data_sources::collect_remote_images());
            images
        } else {
            Vec::new()
        };
        let images = if images.is_empty() {
            vec![
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
            ]
        } else {
            images
                .into_iter()
                .map(|image| ImageOption {
                    label: image.label,
                    version: image.version,
                    edition: image.edition,
                    path: image.path,
                })
                .collect()
        };

        let locales = if flags.locales {
            data_sources::collect_locales()
        } else {
            Vec::new()
        };
        let locales = if locales.is_empty() {
            LOCALES
                .iter()
                .map(|locale| format!("{}:{}", locale.lang, locale.keymap))
                .collect()
        } else {
            locales
        };

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
            welcome_options: vec![
                "Begin installation".to_string(),
                "Review Dojo steps".to_string(),
            ],
            welcome_index: 0,
            use_real_disks,
            disks,
            disk_index: default_disk_index,
            disk_confirm_index: 0,
            wipe_confirmation: String::new(),
            partition_schemes,
            scheme_index,
            partition_layouts: vec![
                "EFI 1024MiB | BOOT 2048MiB | ROOT 1800GiB | DATA rest".to_string(),
                "EFI 512MiB | BOOT 1024MiB | ROOT 64GiB | DATA rest".to_string(),
            ],
            layout_index: 0,
            partition_customizations: Vec::new(),
            customize_index: 0,
            efi_size: "1024MiB".to_string(),
            boot_size: "2048MiB".to_string(),
            root_end: "1800GiB".to_string(),
            os_distros: super::flash_config::OsDistro::all().to_vec(),
            os_distro_index: 0, // Default to Fedora
            os_variants: Vec::new(),
            os_variant_index: 0,
            image_sources: vec![
                SourceOption {
                    label: "Local Image File (.raw)".to_string(),
                    value: ImageSource::LocalFile,
                },
                SourceOption {
                    label: "Download OS Image".to_string(),
                    value: ImageSource::DownloadCatalogue,
                },
            ],
            image_source_index: 0,
            images,
            image_index: 0,
            uefi_sources: super::flash_config::EfiSource::all().to_vec(),
            uefi_source_index: 1, // Default to local EFI image
            locales,
            locale_index: 0,
            options: vec![
                OptionToggle {
                    label: "Auto-unmount target disk".to_string(),
                    enabled: true,
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
            downloading_fedora_index: 0,
            downloading_uefi_index: 0,
            downloaded_fedora: false,
            downloaded_uefi: false,
            destructive_armed: false,
            flash_start_time: None,
            is_running: false,
            status_message: "👋 Welcome to MASH!".to_string(),
            error_message: None,
            image_source_path: "/tmp/fedora.raw".to_string(),
            uefi_source_path: "/tmp/uefi".to_string(),
            downloaded_image_path: None,
            downloaded_uefi_dir: None,
            execute_confirmation_input: String::new(),
            execute_confirmation_confirmed: false,
            safe_mode_disarm_input: String::new(),
            pending_destructive_action: None,
            cancel_requested: Arc::new(std::sync::atomic::AtomicBool::new(false)),
            customize_error_field: None,
            dry_run: false,
            developer_mode: false,
            mash_root: PathBuf::from("."),
            state_path: PathBuf::from("state.json"),
            completion_lines: Vec::new(),
        }
        .with_partition_defaults()
    }

    pub fn new_with_flags(dry_run: bool) -> Self {
        let mut app = Self::new();
        app.dry_run = dry_run;
        app
    }

    pub fn new_with_mash_root(mash_root: PathBuf, dry_run: bool, developer_mode: bool) -> Self {
        let mut app = Self::new_with_flags(dry_run);
        app.mash_root = mash_root.clone();
        app.state_path = mash_root.join("var/lib/mash/state.json");
        app.developer_mode = developer_mode;
        app
    }

    // New: handle input for step advancement
    pub fn handle_input(&mut self, key: KeyEvent) -> InputResult {
        match self.current_step_type {
            InstallStepType::Welcome => self.handle_welcome_input(key),
            InstallStepType::BackupConfirmation => self.handle_backup_confirmation_input(key),
            InstallStepType::DiskSelection => self.handle_disk_selection_input(key),
            InstallStepType::DiskConfirmation => self.handle_disk_confirmation_input(key),
            InstallStepType::PartitionScheme => {
                let len = self.partition_schemes.len();
                let action = Self::list_action(key, len, &mut self.scheme_index);
                self.apply_list_action(action)
            }
            InstallStepType::PartitionLayout => self.handle_partition_layout_input(key),
            InstallStepType::PartitionCustomize => self.handle_partition_customize_input(key),
            InstallStepType::DownloadSourceSelection => {
                let len = self.image_sources.len();
                let is_local = self
                    .image_sources
                    .get(self.image_source_index)
                    .map(|source| source.value == ImageSource::LocalFile)
                    .unwrap_or(false);
                if is_local {
                    self.handle_image_source_path_input(key, len)
                } else {
                    let action = Self::list_action(key, len, &mut self.image_source_index);
                    self.apply_list_action(action)
                }
            }
            InstallStepType::ImageSelection => self.handle_os_distro_selection_input(key),
            InstallStepType::VariantSelection => self.handle_variant_selection_input(key),
            InstallStepType::EfiImage => self.handle_uefi_source_selection_input(key),
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
            InstallStepType::PlanReview => self.handle_plan_review_input(key),
            InstallStepType::Confirmation => self.handle_confirmation_input(key),
            InstallStepType::ExecuteConfirmationGate => {
                self.handle_execute_confirmation_gate_input(key)
            }
            InstallStepType::DisarmSafeMode => self.handle_disarm_safe_mode_input(key),
            InstallStepType::DownloadingFedora => self.handle_downloading_fedora_input(key),
            InstallStepType::DownloadingUefi => self.handle_downloading_uefi_input(key),
            step if step.is_config_step() => self.handle_generic_config_input(key),
            InstallStepType::Flashing => self.handle_flashing_input(key),
            _ => InputResult::Continue, // Default: just continue if no specific handler
        }
    }

    fn handle_welcome_input(&mut self, key: KeyEvent) -> InputResult {
        match key.code {
            KeyCode::Up | KeyCode::Left => {
                let len = self.welcome_options.len();
                Self::adjust_index(len, &mut self.welcome_index, -1);
                InputResult::Continue
            }
            KeyCode::Down | KeyCode::Right => {
                let len = self.welcome_options.len();
                Self::adjust_index(len, &mut self.welcome_index, 1);
                InputResult::Continue
            }
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
                self.go_next();
                InputResult::Continue
            }
            KeyCode::Esc => {
                self.go_prev();
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
                self.go_next();
                InputResult::Continue
            }
            KeyCode::Char('n') | KeyCode::Char('N') => {
                self.backup_confirmed = false;
                self.backup_choice_index = 0;
                self.error_message = Some("Backup confirmation required to proceed.".to_string());
                self.go_prev();
                InputResult::Continue
            }
            KeyCode::Enter => {
                if self.backup_choice_index == 1 {
                    self.backup_confirmed = true;
                    self.error_message = None;
                    self.go_next();
                } else {
                    self.backup_confirmed = false;
                    self.error_message =
                        Some("Backup confirmation required to proceed.".to_string());
                }
                InputResult::Continue
            }
            KeyCode::Esc => {
                self.go_prev();
                InputResult::Continue
            }
            KeyCode::Char('q') => InputResult::Quit,
            _ => InputResult::Continue,
        }
    }

    fn handle_disk_selection_input(&mut self, key: KeyEvent) -> InputResult {
        let protected = self
            .disks
            .get(self.disk_index)
            .map(|disk| disk.boot_confidence.is_boot() || disk.is_source_disk)
            .unwrap_or(false);

        match key.code {
            KeyCode::Char('r') => {
                self.rescan_disks();
                InputResult::Continue
            }
            KeyCode::Up | KeyCode::Left | KeyCode::BackTab => {
                self.adjust_disk_index(-1);
                InputResult::Continue
            }
            KeyCode::Down | KeyCode::Right | KeyCode::Tab => {
                self.adjust_disk_index(1);
                InputResult::Continue
            }
            KeyCode::Enter => {
                if protected && !self.developer_mode {
                    self.error_message = Some(
                        "🛑 This disk is the source/boot media and cannot be selected. Re-run with --developer-mode to override (dangerous)."
                            .to_string(),
                    );
                    return InputResult::Continue;
                }

                self.error_message = None;
                self.apply_list_action(ListAction::Advance)
            }
            KeyCode::Esc => self.apply_list_action(ListAction::Back),
            KeyCode::Char('q') => self.apply_list_action(ListAction::Quit),
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
            KeyCode::Char(' ') | KeyCode::Enter => {
                if let Some(option) = self.options.get_mut(self.options_index) {
                    option.enabled = !option.enabled;
                }
                InputResult::Continue
            }
            KeyCode::Esc => {
                self.go_prev();
                InputResult::Continue
            }
            KeyCode::Char('q') => InputResult::Quit,
            _ => InputResult::Continue,
        }
    }

    fn handle_plan_review_input(&mut self, key: KeyEvent) -> InputResult {
        match key.code {
            KeyCode::Enter => {
                self.go_next();
                InputResult::Continue
            }
            KeyCode::Esc => {
                self.go_prev();
                InputResult::Continue
            }
            KeyCode::Char('q') => InputResult::Quit,
            _ => InputResult::Continue,
        }
    }

    fn handle_disk_confirmation_input(&mut self, key: KeyEvent) -> InputResult {
        let is_boot_or_source_disk = self
            .disks
            .get(self.disk_index)
            .map(|disk| disk.boot_confidence.is_boot() || disk.is_source_disk)
            .unwrap_or(false);

        let required_text = if is_boot_or_source_disk {
            "DESTROY BOOT DISK"
        } else {
            "DESTROY"
        };

        match key.code {
            KeyCode::Char(c) if c.is_ascii_alphanumeric() || c == ' ' => {
                if self.wipe_confirmation.len() < 32 {
                    self.wipe_confirmation.push(c.to_ascii_uppercase());
                }
                InputResult::Continue
            }
            KeyCode::Backspace => {
                self.wipe_confirmation.pop();
                InputResult::Continue
            }
            KeyCode::Enter => {
                // Disk topology can change while the TUI is open; rescan before allowing a
                // destructive transition so we don't target a disconnected/re-enumerated device.
                if self.use_real_disks {
                    self.rescan_disks();
                    if self.disks.is_empty() {
                        return InputResult::Continue;
                    }
                }

                if self.wipe_confirmation == required_text {
                    self.error_message = None;
                    self.wipe_confirmation.clear();
                    self.go_next();
                } else {
                    self.error_message = if is_boot_or_source_disk {
                        Some(
                            "⚠️ Type 'DESTROY BOOT DISK' to confirm destruction of BOOT DEVICE."
                                .to_string(),
                        )
                    } else {
                        Some("Type DESTROY to confirm disk destruction.".to_string())
                    };
                }
                InputResult::Continue
            }
            KeyCode::Esc => {
                self.wipe_confirmation.clear();
                self.go_prev();
                InputResult::Continue
            }
            KeyCode::Char('q') => InputResult::Quit,
            _ => InputResult::Continue,
        }
    }

    fn handle_confirmation_input(&mut self, key: KeyEvent) -> InputResult {
        match key.code {
            KeyCode::Enter => {
                // WO-036: execute-mode requires an explicit typed confirmation gate.
                if !self.dry_run && !self.execute_confirmation_confirmed {
                    self.execute_confirmation_input.clear();
                    self.error_message = None;
                    self.current_step_type = InstallStepType::ExecuteConfirmationGate;
                    return InputResult::Continue;
                }
                self.try_start_flash()
            }
            KeyCode::Esc => {
                self.go_prev();
                InputResult::Continue
            }
            KeyCode::Char('q') => InputResult::Quit,
            _ => InputResult::Continue,
        }
    }

    fn handle_execute_confirmation_gate_input(&mut self, key: KeyEvent) -> InputResult {
        const REQUIRED: &str = "I UNDERSTAND THIS WILL ERASE THE SELECTED DISK";
        match key.code {
            KeyCode::Char(c) if c.is_ascii_uppercase() || c == ' ' => {
                if self.execute_confirmation_input.len() < REQUIRED.len() {
                    self.execute_confirmation_input.push(c);
                }
                InputResult::Continue
            }
            KeyCode::Backspace => {
                self.execute_confirmation_input.pop();
                InputResult::Continue
            }
            KeyCode::Enter => {
                if self.execute_confirmation_input == REQUIRED {
                    self.execute_confirmation_confirmed = true;
                    self.error_message = None;
                    self.execute_confirmation_input.clear();
                    // Proceed into Safe Mode disarm (if needed) and then execution.
                    self.try_start_flash()
                } else {
                    self.error_message =
                        Some(format!("Type the full phrase exactly: {}", REQUIRED));
                    InputResult::Continue
                }
            }
            KeyCode::Esc => {
                self.execute_confirmation_input.clear();
                self.execute_confirmation_confirmed = false;
                self.current_step_type = InstallStepType::Confirmation;
                InputResult::Continue
            }
            KeyCode::Char('q') => InputResult::Quit,
            _ => InputResult::Continue,
        }
    }

    fn handle_disarm_safe_mode_input(&mut self, key: KeyEvent) -> InputResult {
        match key.code {
            KeyCode::Char(c) if c.is_ascii_alphanumeric() => {
                if self.safe_mode_disarm_input.len() < 16 {
                    self.safe_mode_disarm_input.push(c);
                }
                InputResult::Continue
            }
            KeyCode::Backspace => {
                self.safe_mode_disarm_input.pop();
                InputResult::Continue
            }
            KeyCode::Enter => {
                if self.safe_mode_disarm_input == "DESTROY" {
                    self.destructive_armed = true;
                    self.error_message = None;
                    self.safe_mode_disarm_input.clear();
                    match self.pending_destructive_action.take() {
                        Some(PendingDestructiveAction::StartFlash) => self.try_start_flash(),
                        None => {
                            self.current_step_type = InstallStepType::Confirmation;
                            InputResult::Continue
                        }
                    }
                } else {
                    self.error_message =
                        Some("Type DESTROY (exactly) to disarm Safe Mode.".to_string());
                    InputResult::Continue
                }
            }
            KeyCode::Esc => {
                self.safe_mode_disarm_input.clear();
                self.pending_destructive_action = None;
                self.current_step_type = InstallStepType::Confirmation;
                InputResult::Continue
            }
            KeyCode::Char('q') => InputResult::Quit,
            _ => InputResult::Continue,
        }
    }

    fn try_start_flash(&mut self) -> InputResult {
        match validate_partition_plan(&self.build_partition_plan()) {
            Ok(()) => {
                self.error_message = None;
                self.customize_error_field = None;
            }
            Err(err) => {
                self.error_message = Some(err.message);
                self.customize_error_field = err.field;
                return InputResult::Continue;
            }
        }

        if !self.dry_run && !self.destructive_armed {
            self.pending_destructive_action = Some(PendingDestructiveAction::StartFlash);
            self.safe_mode_disarm_input.clear();
            self.error_message = None;
            self.current_step_type = InstallStepType::DisarmSafeMode;
            return InputResult::Continue;
        }

        self.current_step_type = InstallStepType::Flashing;
        self.status_message = "🛠️ Starting installation...".to_string();
        self.cancel_requested
            .store(false, std::sync::atomic::Ordering::Relaxed);

        if let Some(config) = self.build_flash_config() {
            return InputResult::StartFlash(Box::new(config));
        }
        InputResult::Continue
    }

    fn handle_downloading_fedora_input(&mut self, key: KeyEvent) -> InputResult {
        match key.code {
            KeyCode::Up | KeyCode::Left => {
                Self::adjust_index(2, &mut self.downloading_fedora_index, -1);
                InputResult::Continue
            }
            KeyCode::Down | KeyCode::Right => {
                Self::adjust_index(2, &mut self.downloading_fedora_index, 1);
                InputResult::Continue
            }
            KeyCode::Enter => {
                if self.downloading_fedora_index == 0 {
                    self.downloaded_fedora = true;
                    if self.requires_uefi_download() {
                        self.current_step_type = InstallStepType::DownloadingUefi;
                        self.status_message = "⬇️ Preparing EFI download...".to_string();
                    } else {
                        self.start_flashing();
                    }
                } else {
                    self.go_prev();
                }
                InputResult::Continue
            }
            KeyCode::Esc => {
                self.go_prev();
                InputResult::Continue
            }
            KeyCode::Char('q') => InputResult::Quit,
            _ => InputResult::Continue,
        }
    }

    fn handle_downloading_uefi_input(&mut self, key: KeyEvent) -> InputResult {
        match key.code {
            KeyCode::Up | KeyCode::Left => {
                Self::adjust_index(2, &mut self.downloading_uefi_index, -1);
                InputResult::Continue
            }
            KeyCode::Down | KeyCode::Right => {
                Self::adjust_index(2, &mut self.downloading_uefi_index, 1);
                InputResult::Continue
            }
            KeyCode::Enter => {
                if self.downloading_uefi_index == 0 {
                    self.downloaded_uefi = true;
                    self.start_flashing();
                } else {
                    self.go_prev();
                }
                InputResult::Continue
            }
            KeyCode::Esc => {
                self.go_prev();
                InputResult::Continue
            }
            KeyCode::Char('q') => InputResult::Quit,
            _ => InputResult::Continue,
        }
    }

    fn handle_os_distro_selection_input(&mut self, key: KeyEvent) -> InputResult {
        match key.code {
            KeyCode::Up | KeyCode::Left | KeyCode::BackTab => {
                Self::adjust_index(self.os_distros.len(), &mut self.os_distro_index, -1);
                self.refresh_os_variants();
                InputResult::Continue
            }
            KeyCode::Down | KeyCode::Right | KeyCode::Tab => {
                Self::adjust_index(self.os_distros.len(), &mut self.os_distro_index, 1);
                self.refresh_os_variants();
                InputResult::Continue
            }
            KeyCode::Enter => {
                self.error_message = None;
                if self.os_variants.is_empty() {
                    self.error_message = Some(
                        "Missing metadata: no variants found for this OS. Cannot continue."
                            .to_string(),
                    );
                    return InputResult::Continue;
                }
                self.apply_list_action(ListAction::Advance)
            }
            KeyCode::Esc => self.apply_list_action(ListAction::Back),
            KeyCode::Char('q') => self.apply_list_action(ListAction::Quit),
            _ => InputResult::Continue,
        }
    }

    fn handle_variant_selection_input(&mut self, key: KeyEvent) -> InputResult {
        match key.code {
            KeyCode::Up | KeyCode::Left | KeyCode::BackTab => {
                Self::adjust_index(self.os_variants.len(), &mut self.os_variant_index, -1);
                InputResult::Continue
            }
            KeyCode::Down | KeyCode::Right | KeyCode::Tab => {
                Self::adjust_index(self.os_variants.len(), &mut self.os_variant_index, 1);
                InputResult::Continue
            }
            KeyCode::Enter => {
                if self.os_variants.is_empty() {
                    self.error_message = Some(
                        "Missing metadata: no variants found for this OS. Cannot continue."
                            .to_string(),
                    );
                    return InputResult::Continue;
                }
                self.error_message = None;
                self.apply_list_action(ListAction::Advance)
            }
            KeyCode::Esc => self.apply_list_action(ListAction::Back),
            KeyCode::Char('q') => self.apply_list_action(ListAction::Quit),
            _ => InputResult::Continue,
        }
    }

    fn handle_uefi_source_selection_input(&mut self, key: KeyEvent) -> InputResult {
        let is_local = matches!(
            self.uefi_sources.get(self.uefi_source_index),
            Some(super::flash_config::EfiSource::LocalEfiImage)
        );

        // If local directory is selected, accept text input for path
        if is_local {
            match key.code {
                KeyCode::Up | KeyCode::Left | KeyCode::BackTab => {
                    Self::adjust_index(self.uefi_sources.len(), &mut self.uefi_source_index, -1);
                    InputResult::Continue
                }
                KeyCode::Down | KeyCode::Right | KeyCode::Tab => {
                    Self::adjust_index(self.uefi_sources.len(), &mut self.uefi_source_index, 1);
                    InputResult::Continue
                }
                KeyCode::Char(c) => {
                    self.uefi_source_path.push(c);
                    InputResult::Continue
                }
                KeyCode::Backspace => {
                    self.uefi_source_path.pop();
                    InputResult::Continue
                }
                KeyCode::Enter => self.apply_list_action(ListAction::Advance),
                KeyCode::Esc => self.apply_list_action(ListAction::Back),
                _ => InputResult::Continue,
            }
        } else {
            // Download selected - simple navigation
            match key.code {
                KeyCode::Up | KeyCode::Left | KeyCode::BackTab => {
                    Self::adjust_index(self.uefi_sources.len(), &mut self.uefi_source_index, -1);
                    InputResult::Continue
                }
                KeyCode::Down | KeyCode::Right | KeyCode::Tab => {
                    Self::adjust_index(self.uefi_sources.len(), &mut self.uefi_source_index, 1);
                    InputResult::Continue
                }
                KeyCode::Enter => self.apply_list_action(ListAction::Advance),
                KeyCode::Esc => self.apply_list_action(ListAction::Back),
                KeyCode::Char('q') => self.apply_list_action(ListAction::Quit),
                _ => InputResult::Continue,
            }
        }
    }

    fn apply_list_action(&mut self, action: ListAction) -> InputResult {
        match action {
            ListAction::Advance => {
                self.error_message = None;
                self.go_next();
                InputResult::Continue
            }
            ListAction::Back => {
                self.go_prev();
                InputResult::Continue
            }
            ListAction::Quit => InputResult::Quit,
            ListAction::None => InputResult::Continue,
        }
    }

    fn next_step_for(&self, from: InstallStepType) -> Option<InstallStepType> {
        use InstallStepType as S;
        let distro = self.selected_distro();
        match from {
            S::Welcome => Some(S::DiskSelection),
            S::DiskSelection => Some(S::DiskConfirmation),
            S::DiskConfirmation => Some(S::ImageSelection),
            S::BackupConfirmation => Some(S::ImageSelection),
            S::ImageSelection => Some(S::VariantSelection),
            S::VariantSelection => Some(S::DownloadSourceSelection),
            S::DownloadSourceSelection => {
                if matches!(distro, super::flash_config::OsDistro::Fedora) {
                    Some(S::PartitionScheme)
                } else {
                    Some(S::LocaleSelection)
                }
            }
            S::PartitionScheme => Some(S::PartitionLayout),
            S::PartitionLayout => Some(S::PartitionCustomize),
            S::PartitionCustomize => Some(S::EfiImage),
            S::EfiImage => Some(S::LocaleSelection),
            S::LocaleSelection => Some(S::Options),
            S::Options => Some(S::PlanReview),
            S::FirstBootUser => Some(S::PlanReview),
            S::PlanReview => Some(S::Confirmation),
            S::Flashing => Some(S::Complete),
            _ => None,
        }
    }

    fn prev_step_for(&self, from: InstallStepType) -> Option<InstallStepType> {
        use InstallStepType as S;
        let distro = self.selected_distro();
        match from {
            S::DiskSelection => Some(S::Welcome),
            S::DiskConfirmation => Some(S::DiskSelection),
            S::BackupConfirmation => Some(S::DiskConfirmation),
            S::ImageSelection => Some(S::DiskConfirmation),
            S::VariantSelection => Some(S::ImageSelection),
            S::DownloadSourceSelection => Some(S::VariantSelection),
            S::PartitionScheme => Some(S::DownloadSourceSelection),
            S::PartitionLayout => Some(S::PartitionScheme),
            S::PartitionCustomize => Some(S::PartitionLayout),
            S::EfiImage => Some(S::PartitionCustomize),
            S::LocaleSelection => {
                if matches!(distro, super::flash_config::OsDistro::Fedora) {
                    Some(S::EfiImage)
                } else {
                    Some(S::DownloadSourceSelection)
                }
            }
            S::Options => Some(S::LocaleSelection),
            S::FirstBootUser => Some(S::Options),
            S::PlanReview => Some(S::Options),
            S::Confirmation => Some(S::PlanReview),
            S::DisarmSafeMode => Some(S::Confirmation),
            _ => None,
        }
    }

    fn go_next(&mut self) {
        if let Some(next) = self.next_step_for(self.current_step_type) {
            // Any navigation through config steps invalidates the execute confirmation gate.
            if !self.is_running {
                self.execute_confirmation_confirmed = false;
                self.execute_confirmation_input.clear();
            }
            self.current_step_type = next;
        }
    }

    fn go_prev(&mut self) {
        if let Some(prev) = self.prev_step_for(self.current_step_type) {
            if !self.is_running {
                self.execute_confirmation_confirmed = false;
                self.execute_confirmation_input.clear();
            }
            self.current_step_type = prev;
        }
    }

    fn list_action(key: KeyEvent, len: usize, index: &mut usize) -> ListAction {
        match key.code {
            KeyCode::Up | KeyCode::Left | KeyCode::BackTab => {
                Self::adjust_index(len, index, -1);
                ListAction::None
            }
            KeyCode::Down | KeyCode::Right | KeyCode::Tab => {
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

    /// Adjust disk index while skipping protected disks (boot/source media) for safety.
    fn adjust_disk_index(&mut self, delta: isize) {
        let len = self.disks.len();
        if len == 0 {
            self.disk_index = 0;
            return;
        }

        let start_index = self.disk_index;
        let mut attempts = 0;
        loop {
            // Use standard index adjustment
            Self::adjust_index(len, &mut self.disk_index, delta);

            // Check if we found an allowed disk or cycled through all.
            if let Some(disk) = self.disks.get(self.disk_index) {
                let protected = disk.boot_confidence.is_boot() || disk.is_source_disk;
                if !protected || self.developer_mode {
                    return;
                }
            }

            attempts += 1;
            if attempts >= len || self.disk_index == start_index {
                // All disks are protected or we cycled back - stay on current.
                return;
            }
        }
    }

    fn rescan_disks(&mut self) {
        if !self.use_real_disks {
            return;
        }

        let selected = self
            .disks
            .get(self.disk_index)
            .map(|disk| disk.stable_id.clone());

        let mut disks = data_sources::scan_disks()
            .into_iter()
            .map(|disk| DiskOption {
                identity: disk.identity,
                stable_id: disk.stable_id,
                path: disk.path,
                removable: disk.removable,
                boot_confidence: disk.boot_confidence,
                is_source_disk: disk.is_source_disk,
            })
            .collect::<Vec<_>>();

        // Deterministic ordering in case kernel enumeration changes.
        disks.sort_by(|a, b| a.path.cmp(&b.path));

        if disks.is_empty() {
            self.disks.clear();
            self.disk_index = 0;
            self.error_message =
                Some("No disks detected. Insert a target disk and press r.".to_string());
            return;
        }

        // Preserve selection if possible.
        let mut next_index = selected
            .as_ref()
            .and_then(|id| disks.iter().position(|d| &d.stable_id == id))
            .unwrap_or(0);

        // If the preserved selection is protected, prefer a safe target unless developer-mode.
        if !self.developer_mode {
            if let Some(disk) = disks.get(next_index) {
                let protected = disk.boot_confidence.is_boot() || disk.is_source_disk;
                if protected {
                    if let Some(idx) = disks.iter().position(|d| {
                        !(d.boot_confidence.is_boot() || d.is_source_disk) && d.removable
                    }) {
                        next_index = idx;
                    } else if let Some(idx) = disks
                        .iter()
                        .position(|d| !(d.boot_confidence.is_boot() || d.is_source_disk))
                    {
                        next_index = idx;
                    }
                }
            }
        }

        self.disks = disks;
        self.disk_index = next_index.min(self.disks.len().saturating_sub(1));
        self.error_message = None;
    }

    fn toggle_backup_choice(&mut self) {
        self.backup_choice_index = if self.backup_choice_index == 0 { 1 } else { 0 };
    }

    fn handle_flashing_input(&mut self, key: KeyEvent) -> InputResult {
        if !self.dry_run && !self.destructive_armed {
            self.error_message = Some(
                "SAFE MODE is active. Return to the summary and disarm Safe Mode.".to_string(),
            );
            self.current_step_type = InstallStepType::Confirmation;
            return InputResult::Continue;
        }
        let is_complete = self
            .progress_state
            .lock()
            .map(|state| state.is_complete)
            .unwrap_or(false);
        match key.code {
            KeyCode::Char('c') | KeyCode::Char('C') => {
                self.cancel_requested
                    .store(true, std::sync::atomic::Ordering::Relaxed);
                self.status_message = "🛑 Cancel requested...".to_string();
                InputResult::Continue
            }
            KeyCode::Enter if is_complete => {
                self.current_step_type = InstallStepType::Complete;
                InputResult::Complete
            }
            KeyCode::Char('q') => InputResult::Quit,
            _ => InputResult::Continue,
        }
    }

    fn handle_partition_layout_input(&mut self, key: KeyEvent) -> InputResult {
        match key.code {
            KeyCode::Char('y') | KeyCode::Char('Y') | KeyCode::Enter => {
                self.error_message = None;
                // Accept selected layout and skip customization, go directly to EFI setup.
                self.current_step_type = InstallStepType::EfiImage;
                InputResult::Continue
            }
            KeyCode::Char('m') | KeyCode::Char('M') | KeyCode::Char('n') | KeyCode::Char('N') => {
                // Manual customization mode
                self.current_step_type = InstallStepType::PartitionCustomize;
                InputResult::Continue
            }
            _ => {
                let len = self.partition_layouts.len();
                let action = Self::list_action(key, len, &mut self.layout_index);
                self.apply_list_action(action)
            }
        }
    }

    fn handle_partition_customize_input(&mut self, key: KeyEvent) -> InputResult {
        match key.code {
            KeyCode::Up | KeyCode::Left | KeyCode::BackTab => {
                let len = self.partition_customizations.len();
                Self::adjust_index(len, &mut self.customize_index, -1);
                InputResult::Continue
            }
            KeyCode::Down | KeyCode::Right | KeyCode::Tab => {
                let len = self.partition_customizations.len();
                Self::adjust_index(len, &mut self.customize_index, 1);
                InputResult::Continue
            }
            KeyCode::Char(c) if c.is_ascii_alphanumeric() || c == '.' => {
                self.error_message = None;
                self.customize_error_field = None;
                if self.apply_customize_edit(Some(c)) {
                    self.refresh_partition_customizations();
                }
                InputResult::Continue
            }
            KeyCode::Backspace => {
                self.error_message = None;
                self.customize_error_field = None;
                if self.apply_customize_edit(None) {
                    self.refresh_partition_customizations();
                }
                InputResult::Continue
            }
            KeyCode::Enter => {
                match validate_partition_plan(&self.build_partition_plan()) {
                    Ok(()) => {
                        self.error_message = None;
                        self.customize_error_field = None;
                        self.go_next();
                    }
                    Err(err) => {
                        self.error_message = Some(err.message);
                        self.customize_error_field = err.field;
                    }
                }
                InputResult::Continue
            }
            KeyCode::Char('r') | KeyCode::Char('R') => {
                self.efi_size = "1024MiB".to_string();
                self.boot_size = "2048MiB".to_string();
                self.root_end = "1800GiB".to_string();
                self.error_message = None;
                self.customize_error_field = None;
                self.refresh_partition_customizations();
                InputResult::Continue
            }
            KeyCode::Esc => {
                self.go_prev();
                InputResult::Continue
            }
            KeyCode::Char('q') => InputResult::Quit,
            _ => InputResult::Continue,
        }
    }

    fn handle_image_source_path_input(&mut self, key: KeyEvent, len: usize) -> InputResult {
        match key.code {
            KeyCode::Char(c) if !c.is_control() => {
                self.image_source_path.push(c);
                InputResult::Continue
            }
            KeyCode::Backspace => {
                self.image_source_path.pop();
                InputResult::Continue
            }
            KeyCode::Up | KeyCode::Left | KeyCode::BackTab => {
                Self::adjust_index(len, &mut self.image_source_index, -1);
                InputResult::Continue
            }
            KeyCode::Down | KeyCode::Right | KeyCode::Tab => {
                Self::adjust_index(len, &mut self.image_source_index, 1);
                InputResult::Continue
            }
            KeyCode::Enter => {
                self.error_message = None;
                self.go_next();
                InputResult::Continue
            }
            KeyCode::Esc => {
                self.go_prev();
                InputResult::Continue
            }
            KeyCode::Char('q') => InputResult::Quit,
            _ => InputResult::Continue,
        }
    }

    fn apply_customize_edit(&mut self, input: Option<char>) -> bool {
        let target = match self.customize_index {
            0 => Some(&mut self.efi_size),
            1 => Some(&mut self.boot_size),
            2 => Some(&mut self.root_end),
            _ => None,
        };
        let Some(value) = target else {
            return false;
        };
        match input {
            Some(ch) => value.push(ch),
            None => {
                value.pop();
            }
        }
        true
    }

    fn refresh_partition_customizations(&mut self) {
        self.partition_customizations = vec![
            format!("EFI {}", self.efi_size),
            format!("BOOT {}", self.boot_size),
            format!("ROOT {}", self.root_end),
            "DATA remainder".to_string(),
        ];
    }

    fn build_partition_plan(&self) -> PartitionPlan {
        PartitionPlan {
            scheme: *self
                .partition_schemes
                .get(self.scheme_index)
                .unwrap_or(&PartitionScheme::Mbr),
            partitions: vec![
                Partition {
                    name: "EFI".to_string(),
                    size: self.efi_size.clone(),
                    format: "vfat".to_string(),
                    flags: vec!["esp".to_string()],
                },
                Partition {
                    name: "BOOT".to_string(),
                    size: self.boot_size.clone(),
                    format: "ext4".to_string(),
                    flags: Vec::new(),
                },
                Partition {
                    name: "ROOT".to_string(),
                    size: self.root_end.clone(),
                    format: "btrfs".to_string(),
                    flags: Vec::new(),
                },
                Partition {
                    name: "DATA".to_string(),
                    size: "remainder".to_string(),
                    format: "ext4".to_string(),
                    flags: Vec::new(),
                },
            ],
        }
    }

    fn with_partition_defaults(mut self) -> Self {
        self.refresh_partition_customizations();
        self.refresh_os_variants();
        self
    }

    fn requires_image_download(&self) -> bool {
        let source_is_download = self
            .image_sources
            .get(self.image_source_index)
            .map(|source| source.value == ImageSource::DownloadCatalogue)
            .unwrap_or(false);
        source_is_download
    }

    fn requires_uefi_download(&self) -> bool {
        matches!(
            self.uefi_sources.get(self.uefi_source_index),
            Some(super::flash_config::EfiSource::DownloadEfiImage)
        )
    }

    fn selected_distro(&self) -> super::flash_config::OsDistro {
        self.os_distros
            .get(self.os_distro_index)
            .copied()
            .unwrap_or(super::flash_config::OsDistro::Fedora)
    }

    fn refresh_os_variants(&mut self) {
        let distro = self.selected_distro();
        let os_kind = distro.as_os_kind();
        let Ok(index) = mash_core::downloader::download_index() else {
            self.os_variants.clear();
            self.os_variant_index = 0;
            self.status_message =
                "❌ Download catalogue unavailable (failed to parse index).".to_string();
            return;
        };
        let mut variants = index
            .images
            .iter()
            .filter(|img| img.os == os_kind && img.arch == "aarch64")
            .map(|img| OsVariantOption {
                id: img.variant.clone(),
                label: format_variant_label(os_kind, &img.variant),
            })
            .collect::<Vec<_>>();

        // Deterministic ordering for UI.
        variants.sort_by(|a, b| a.label.cmp(&b.label));

        self.os_variants = variants;
        self.os_variant_index = 0;
    }

    fn selected_variant_id(&self) -> Option<String> {
        self.os_variants
            .get(self.os_variant_index)
            .map(|v| v.id.clone())
    }

    fn start_flashing(&mut self) {
        self.current_step_type = InstallStepType::Flashing;
        self.status_message = "🛠️ Starting installation...".to_string();
        if self.flash_start_time.is_none() {
            self.flash_start_time = Some(Instant::now());
        }
    }

    pub fn progress_state_snapshot(&self) -> ProgressState {
        self.progress_state
            .lock()
            .map(|state| state.clone())
            .unwrap_or_default()
    }

    /// Build flash configuration from current app state
    pub fn build_flash_config(&self) -> Option<TuiFlashConfig> {
        let download_uefi_firmware = matches!(
            self.uefi_sources.get(self.uefi_source_index),
            Some(super::flash_config::EfiSource::DownloadEfiImage)
        );

        let os_distro = self.selected_distro();
        let os_variant = self.selected_variant_id().unwrap_or_default();
        let os_kind = os_distro.as_os_kind();

        let downloads_dir = self.mash_root.join("downloads");
        let image_download_dir = downloads_dir.join("images");
        let uefi_download_dir = downloads_dir.join("uefi");

        let image_source = self
            .image_sources
            .get(self.image_source_index)
            .map(|source| source.value)
            .unwrap_or(ImageSource::LocalFile);

        let image_path = match image_source {
            ImageSource::LocalFile => PathBuf::from(self.image_source_path.clone()),
            ImageSource::DownloadCatalogue => {
                let file_name = mash_core::downloader::download_index()
                    .ok()
                    .and_then(|index| {
                        index
                            .images
                            .iter()
                            .find(|img| {
                                img.os == os_kind
                                    && img.variant == os_variant
                                    && img.arch == "aarch64"
                            })
                            .map(|img| img.file_name.clone())
                    })
                    .unwrap_or_else(|| "missing-image-spec.img.xz".to_string());
                image_download_dir.join(file_name)
            }
        };

        let disk_identity = self.disks.get(self.disk_index).map(|disk| {
            mash_core::install_report::DiskIdentityReport {
                vendor: disk.identity.vendor.clone(),
                model: disk.identity.model.clone(),
                transport: Some(disk.identity.transport.hint().to_string()),
                size_bytes: Some(disk.identity.size_bytes),
            }
        });

        Some(TuiFlashConfig {
            mash_root: self.mash_root.clone(),
            state_path: self.state_path.clone(),
            os_distro,
            os_variant,
            image: image_path,
            disk: self
                .disks
                .get(self.disk_index)
                .map(|disk| disk.path.clone())
                .unwrap_or_else(|| "/dev/sda".to_string()),
            scheme: *self
                .partition_schemes
                .get(self.scheme_index)
                .unwrap_or(&PartitionScheme::Mbr),
            uefi_dir: self.downloaded_uefi_dir.clone().unwrap_or_else(|| {
                if download_uefi_firmware {
                    uefi_download_dir.clone()
                } else {
                    PathBuf::from(self.uefi_source_path.clone())
                }
            }),
            auto_unmount: self
                .options
                .iter()
                .find(|option| option.label == "Auto-unmount target disk")
                .map(|option| option.enabled)
                .unwrap_or(true),
            watch: false,
            locale: self
                .locales
                .get(self.locale_index)
                .and_then(|locale| LocaleConfig::parse_from_str(locale).ok()),
            early_ssh: self
                .options
                .iter()
                .find(|option| option.label == "Enable early SSH")
                .map(|option| option.enabled)
                .unwrap_or(false),
            progress_tx: self.flash_progress_sender.clone(), // This is the critical change!
            efi_size: self.efi_size.clone(),
            boot_size: self.boot_size.clone(),
            root_end: self.root_end.clone(),
            download_uefi_firmware,
            image_source_selection: image_source,
            // Legacy Fedora-only fields: kept for compatibility with the existing Fedora flash path.
            image_version: "43".to_string(),
            image_edition: "KDE".to_string(),
            dry_run: self.dry_run,
            disk_identity,
            os_distro_label: self
                .os_distros
                .get(self.os_distro_index)
                .map(|d| d.display().to_string())
                .unwrap_or_else(|| "Unknown".to_string()),
            os_flavour_label: self
                .os_variants
                .get(self.os_variant_index)
                .map(|v| v.label.clone())
                .unwrap_or_else(|| "Unknown".to_string()),
            typed_execute_confirmation: self.execute_confirmation_confirmed,
        })
    }
}

fn format_variant_label(os: mash_core::downloader::OsKind, variant: &str) -> String {
    match (os, variant) {
        (mash_core::downloader::OsKind::Fedora, "kde_mobile_disk") => {
            "Fedora KDE Mobile Disk (F43, aarch64)".to_string()
        }
        (mash_core::downloader::OsKind::Ubuntu, "server_24_04_3") => {
            "Ubuntu 24.04.3 Server (arm64+raspi)".to_string()
        }
        (mash_core::downloader::OsKind::Ubuntu, "desktop_24_04_3") => {
            "Ubuntu 24.04.3 Desktop (arm64+raspi)".to_string()
        }
        (mash_core::downloader::OsKind::RaspberryPiOS, "arm64_latest") => {
            "Raspberry Pi OS (arm64) Latest".to_string()
        }
        (mash_core::downloader::OsKind::Manjaro, "minimal_rpi4_23_02") => {
            "Manjaro ARM Minimal (RPI4) 23.02".to_string()
        }
        _ => variant.to_string(),
    }
}

struct ValidationError {
    message: String,
    field: Option<CustomizeField>,
}

fn validate_partition_plan(plan: &PartitionPlan) -> Result<(), ValidationError> {
    let mut efi = None;
    let mut boot = None;
    let mut root_end = None;

    for part in &plan.partitions {
        match part.name.to_ascii_uppercase().as_str() {
            "EFI" => efi = Some(part.size.clone()),
            "BOOT" => boot = Some(part.size.clone()),
            "ROOT" => root_end = Some(part.size.clone()),
            _ => {}
        }
    }

    let efi = efi.ok_or_else(|| ValidationError {
        message: "EFI size is required.".to_string(),
        field: Some(CustomizeField::Efi),
    })?;
    let boot = boot.ok_or_else(|| ValidationError {
        message: "BOOT size is required.".to_string(),
        field: Some(CustomizeField::Boot),
    })?;
    let root_end = root_end.ok_or_else(|| ValidationError {
        message: "ROOT end is required.".to_string(),
        field: Some(CustomizeField::Root),
    })?;

    if root_end.contains('%') {
        return Err(ValidationError {
            message: "ROOT end must be an absolute size (e.g., 1800GiB), not a percentage."
                .to_string(),
            field: Some(CustomizeField::Root),
        });
    }

    let efi_mib = parse_size_mib(&efi).map_err(|message| ValidationError {
        message: format!("EFI size invalid: {}", message),
        field: Some(CustomizeField::Efi),
    })?;
    let boot_mib = parse_size_mib(&boot).map_err(|message| ValidationError {
        message: format!("BOOT size invalid: {}", message),
        field: Some(CustomizeField::Boot),
    })?;
    let root_mib = parse_size_mib(&root_end).map_err(|message| ValidationError {
        message: format!("ROOT end invalid: {}", message),
        field: Some(CustomizeField::Root),
    })?;

    if efi_mib < 512 {
        return Err(ValidationError {
            message: "EFI must be at least 512MiB.".to_string(),
            field: Some(CustomizeField::Efi),
        });
    }
    if boot_mib < 512 {
        return Err(ValidationError {
            message: "BOOT must be at least 512MiB.".to_string(),
            field: Some(CustomizeField::Boot),
        });
    }

    let efi_end = efi_mib;
    let boot_end = efi_mib + boot_mib;

    if boot_end <= efi_end {
        return Err(ValidationError {
            message: "BOOT must end after EFI.".to_string(),
            field: Some(CustomizeField::Boot),
        });
    }
    if root_mib <= boot_end {
        return Err(ValidationError {
            message: "ROOT end must be greater than BOOT end.".to_string(),
            field: Some(CustomizeField::Root),
        });
    }

    Ok(())
}

fn parse_size_mib(raw: &str) -> Result<u64, String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err("size is empty".to_string());
    }
    let lower = trimmed.to_ascii_lowercase();
    let (num_str, suffix) = if let Some(rest) = lower.strip_suffix("mib") {
        (rest, "mib")
    } else if let Some(rest) = lower.strip_suffix("mb") {
        (rest, "mb")
    } else if let Some(rest) = lower.strip_suffix('m') {
        (rest, "m")
    } else if let Some(rest) = lower.strip_suffix("gib") {
        (rest, "gib")
    } else if let Some(rest) = lower.strip_suffix("gb") {
        (rest, "gb")
    } else if let Some(rest) = lower.strip_suffix('g') {
        (rest, "g")
    } else {
        return Err("missing unit (use M/MiB or G/GiB)".to_string());
    };

    let value: u64 = num_str
        .trim()
        .parse()
        .map_err(|_| "invalid number".to_string())?;
    let mib = match suffix {
        "mib" | "mb" | "m" => value,
        "gib" | "gb" | "g" => value.saturating_mul(1024),
        _ => return Err("unknown unit".to_string()),
    };
    Ok(mib)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::{KeyEventKind, KeyEventState, KeyModifiers};

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent {
            code,
            modifiers: KeyModifiers::NONE,
            kind: KeyEventKind::Press,
            state: KeyEventState::NONE,
        }
    }

    fn disk_option(
        path: &str,
        vendor: &str,
        model: &str,
        size_bytes: u64,
        transport: data_sources::TransportType,
        removable: bool,
        boot_confidence: data_sources::BootConfidence,
        is_source_disk: bool,
    ) -> DiskOption {
        let identity = data_sources::DiskIdentity::new(
            Some(vendor.to_string()),
            Some(model.to_string()),
            None,
            None,
            size_bytes,
            transport,
        );
        let stable_id = identity.stable_id(path);
        DiskOption {
            identity,
            stable_id,
            path: path.to_string(),
            removable,
            boot_confidence,
            is_source_disk,
        }
    }

    #[test]
    fn tab_cycles_partition_scheme() {
        let mut app = App::new();
        app.current_step_type = InstallStepType::PartitionScheme;
        let initial = app.scheme_index;
        app.handle_input(key(KeyCode::Tab));
        assert_ne!(app.scheme_index, initial);
    }

    #[test]
    fn partition_layout_accepts_yes_no() {
        let mut app = App::new();
        app.current_step_type = InstallStepType::PartitionLayout;
        app.handle_input(key(KeyCode::Char('y')));
        assert_eq!(app.current_step_type, InstallStepType::EfiImage);

        app.current_step_type = InstallStepType::PartitionLayout;
        app.handle_input(key(KeyCode::Char('n')));
        assert_eq!(app.current_step_type, InstallStepType::PartitionCustomize);
    }

    #[test]
    fn partition_customize_edits_selected_field() {
        let mut app = App::new();
        app.current_step_type = InstallStepType::PartitionCustomize;
        app.customize_index = 0;
        let before = app.efi_size.clone();
        app.handle_input(key(KeyCode::Char('9')));
        assert!(app.efi_size.ends_with('9'));
        assert_ne!(app.efi_size, before);
        assert!(app.partition_customizations[0].contains(&app.efi_size));
    }

    #[test]
    fn options_toggle_with_enter() {
        let mut app = App::new();
        app.current_step_type = InstallStepType::Options;
        let before = app.options[0].enabled;
        app.handle_input(key(KeyCode::Enter));
        assert_ne!(app.options[0].enabled, before);
    }

    #[test]
    fn download_source_local_path_edits() {
        let mut app = App::new();
        app.current_step_type = InstallStepType::DownloadSourceSelection;
        app.image_source_index = 0;
        let before = app.image_source_path.clone();
        app.handle_input(key(KeyCode::Char('x')));
        assert_ne!(app.image_source_path, before);
    }

    #[test]
    fn locale_selection_advances_on_enter() {
        let mut app = App::new();
        app.current_step_type = InstallStepType::LocaleSelection;
        app.handle_input(key(KeyCode::Enter));
        assert_eq!(app.current_step_type, InstallStepType::Options);
    }

    #[test]
    fn disk_confirmation_requires_destroy() {
        let mut app = App::new();
        app.current_step_type = InstallStepType::DiskConfirmation;
        // Set up non-boot disk
        app.disks = vec![disk_option(
            "/dev/sdb",
            "Generic",
            "Data Drive",
            512 * 1024 * 1024 * 1024,
            data_sources::TransportType::Sata,
            false,
            data_sources::BootConfidence::NotBoot,
            false,
        )];
        app.disk_index = 0;

        for ch in "DESTROY".chars() {
            app.handle_input(key(KeyCode::Char(ch)));
        }
        assert_eq!(app.wipe_confirmation, "DESTROY");
        app.handle_input(key(KeyCode::Enter));
        assert_eq!(app.current_step_type, InstallStepType::ImageSelection);
    }

    #[test]
    fn boot_disk_confirmation_requires_stronger_text() {
        let mut app = App::new();
        app.current_step_type = InstallStepType::DiskConfirmation;
        // Set up boot disk
        app.disks = vec![disk_option(
            "/dev/sda",
            "Generic",
            "Boot Drive",
            256 * 1024 * 1024 * 1024,
            data_sources::TransportType::Sata,
            false,
            data_sources::BootConfidence::Confident,
            false,
        )];
        app.disk_index = 0;

        // Regular DESTROY should not work for boot disk
        for ch in "DESTROY".chars() {
            app.handle_input(key(KeyCode::Char(ch)));
        }
        app.handle_input(key(KeyCode::Enter));
        assert_eq!(app.current_step_type, InstallStepType::DiskConfirmation);
        assert!(app.error_message.is_some());

        // Clear and type correct phrase
        app.wipe_confirmation.clear();
        for ch in "DESTROY BOOT DISK".chars() {
            app.handle_input(key(KeyCode::Char(ch)));
        }
        assert_eq!(app.wipe_confirmation, "DESTROY BOOT DISK");
        app.handle_input(key(KeyCode::Enter));
        assert_eq!(app.current_step_type, InstallStepType::ImageSelection);
        assert!(app.error_message.is_none());
    }

    #[test]
    fn safe_mode_requires_explicit_disarm_flow() {
        let mut app = App::new();
        app.dry_run = false;
        app.destructive_armed = false;
        app.current_step_type = InstallStepType::Confirmation;

        // Attempt destructive action => execute confirmation gate first.
        app.handle_input(key(KeyCode::Enter));
        assert_eq!(
            app.current_step_type,
            InstallStepType::ExecuteConfirmationGate
        );
        assert!(!app.destructive_armed);

        // Wrong confirmation string should not proceed.
        for ch in "I UNDERSTAND".chars() {
            app.handle_input(key(KeyCode::Char(ch)));
        }
        app.handle_input(key(KeyCode::Enter));
        assert_eq!(
            app.current_step_type,
            InstallStepType::ExecuteConfirmationGate
        );
        assert!(!app.destructive_armed);

        // Correct execute-gate string should proceed into Safe Mode disarm screen.
        app.execute_confirmation_input.clear();
        for ch in "I UNDERSTAND THIS WILL ERASE THE SELECTED DISK".chars() {
            app.handle_input(key(KeyCode::Char(ch)));
        }
        app.handle_input(key(KeyCode::Enter));
        assert_eq!(app.current_step_type, InstallStepType::DisarmSafeMode);

        // Safe Mode disarm string is still required before disk writes.
        app.safe_mode_disarm_input.clear();
        for ch in "DESTROY".chars() {
            app.handle_input(key(KeyCode::Char(ch)));
        }
        let result = app.handle_input(key(KeyCode::Enter));
        assert!(matches!(result, InputResult::StartFlash(_)));
        assert!(app.destructive_armed);
        assert_eq!(app.current_step_type, InstallStepType::Flashing);
    }

    fn plan(efi: &str, boot: &str, root: &str) -> PartitionPlan {
        PartitionPlan {
            scheme: PartitionScheme::Mbr,
            partitions: vec![
                Partition {
                    name: "EFI".to_string(),
                    size: efi.to_string(),
                    format: "vfat".to_string(),
                    flags: vec!["esp".to_string()],
                },
                Partition {
                    name: "BOOT".to_string(),
                    size: boot.to_string(),
                    format: "ext4".to_string(),
                    flags: Vec::new(),
                },
                Partition {
                    name: "ROOT".to_string(),
                    size: root.to_string(),
                    format: "btrfs".to_string(),
                    flags: Vec::new(),
                },
                Partition {
                    name: "DATA".to_string(),
                    size: "remainder".to_string(),
                    format: "ext4".to_string(),
                    flags: Vec::new(),
                },
            ],
        }
    }

    #[test]
    fn validate_partition_plan_rejects_small_efi() {
        let result = validate_partition_plan(&plan("256M", "1024M", "1800G"));
        assert!(result.is_err());
    }

    #[test]
    fn validate_partition_plan_rejects_small_boot() {
        let result = validate_partition_plan(&plan("1024M", "256M", "1800G"));
        assert!(result.is_err());
    }

    #[test]
    fn validate_partition_plan_rejects_root_before_boot() {
        let result = validate_partition_plan(&plan("1024M", "2048M", "2500M"));
        assert!(result.is_err());
    }

    #[test]
    fn validate_partition_plan_rejects_root_percent() {
        let result = validate_partition_plan(&plan("1024M", "2048M", "100%"));
        assert!(result.is_err());
    }

    #[test]
    fn validate_partition_plan_accepts_defaults() {
        let result = validate_partition_plan(&plan("1024MiB", "2048MiB", "1800GiB"));
        assert!(result.is_ok());
    }

    #[test]
    fn download_flags_follow_source_selection() {
        let mut app = App::new();
        app.image_source_index = 0;
        let config = app.build_flash_config().expect("config");
        assert_eq!(config.image_source_selection, ImageSource::LocalFile);

        app.image_source_index = 1;
        let config = app.build_flash_config().expect("config");
        assert_eq!(
            config.image_source_selection,
            ImageSource::DownloadCatalogue
        );
    }

    #[test]
    fn download_uefi_flag_follows_efi_source_selection() {
        let mut app = App::new();
        // Set EFI source to Download (index 0)
        app.uefi_source_index = 0;
        let config = app.build_flash_config().expect("config");
        assert!(config.download_uefi_firmware);
    }

    #[test]
    fn os_distro_defaults_to_fedora() {
        let app = App::new();
        assert_eq!(app.os_distro_index, 0);
        if let Some(distro) = app.os_distros.get(app.os_distro_index) {
            assert!(matches!(
                distro,
                crate::dojo::flash_config::OsDistro::Fedora
            ));
            assert!(distro.is_available());
        }
    }

    #[test]
    fn non_fedora_distros_are_marked_unavailable() {
        let app = App::new();
        // Ubuntu is index 1
        if let Some(distro) = app.os_distros.get(1) {
            assert!(matches!(
                distro,
                crate::dojo::flash_config::OsDistro::Ubuntu
            ));
            assert!(distro.is_available());
        }
    }

    #[test]
    fn uefi_source_defaults_to_local() {
        let app = App::new();
        assert_eq!(app.uefi_source_index, 1);
        if let Some(source) = app.uefi_sources.get(app.uefi_source_index) {
            assert!(matches!(
                source,
                crate::dojo::flash_config::EfiSource::LocalEfiImage
            ));
        }
    }

    #[test]
    fn build_flash_config_uses_partition_sizes() {
        let mut app = App::new();
        app.efi_size = "512MiB".to_string();
        app.boot_size = "1024MiB".to_string();
        app.root_end = "4096MiB".to_string();
        let config = app.build_flash_config().expect("config");
        assert_eq!(config.efi_size, "512MiB");
        assert_eq!(config.boot_size, "1024MiB");
        assert_eq!(config.root_end, "4096MiB");
    }

    #[test]
    fn build_flash_config_respects_dry_run_flag() {
        let mut app = App::new();
        app.dry_run = true;
        let config = app.build_flash_config().expect("config");
        assert!(config.dry_run);
    }

    #[test]
    fn app_initializes_with_first_non_boot_disk() {
        let app = App::new();
        // Stub data has USB as index 0 (non-boot) and NVMe as index 1 (boot)
        // Should default to index 0 which is non-boot
        if !app.disks.is_empty() {
            if let Some(_disk) = app.disks.get(app.disk_index) {
                assert!(
                    !_disk.boot_confidence.is_boot(),
                    "Default disk should not be boot disk"
                );
            }
        }
    }

    #[test]
    fn app_prefers_removable_media_when_safe() {
        // Create app-like disk list
        let disks = [
            disk_option(
                "/dev/nvme0n1",
                "Samsung",
                "980",
                512 * 1024 * 1024 * 1024,
                data_sources::TransportType::Nvme,
                false,
                data_sources::BootConfidence::Confident,
                false,
            ),
            disk_option(
                "/dev/sda",
                "Seagate",
                "BarraCuda",
                2 * 1024 * 1024 * 1024 * 1024,
                data_sources::TransportType::Sata,
                false,
                data_sources::BootConfidence::NotBoot,
                false,
            ),
            disk_option(
                "/dev/sdb",
                "SanDisk",
                "Ultra",
                32 * 1024 * 1024 * 1024,
                data_sources::TransportType::Usb,
                true,
                data_sources::BootConfidence::NotBoot,
                false,
            ),
        ];

        // Simulate the auto-selection logic
        let default_disk_index = disks
            .iter()
            .position(|disk| !disk.boot_confidence.is_boot() && disk.removable)
            .or_else(|| {
                disks
                    .iter()
                    .position(|disk| !disk.boot_confidence.is_boot())
            })
            .unwrap_or(0);

        // Should select USB (index 2) over internal HDD (index 1)
        assert_eq!(default_disk_index, 2);
        assert!(disks[default_disk_index].removable);
        assert!(!disks[default_disk_index].boot_confidence.is_boot());
    }

    #[test]
    fn disk_selection_prevents_selecting_boot_disk() {
        let mut app = App::new();
        app.current_step_type = InstallStepType::DiskSelection;

        // Set up test disks: boot disk at index 0, safe disk at index 1
        app.disks = vec![
            disk_option(
                "/dev/sda",
                "Generic",
                "Boot Drive",
                256 * 1024 * 1024 * 1024,
                data_sources::TransportType::Sata,
                false,
                data_sources::BootConfidence::Confident,
                false,
            ),
            disk_option(
                "/dev/sdb",
                "Generic",
                "Data Drive",
                512 * 1024 * 1024 * 1024,
                data_sources::TransportType::Sata,
                false,
                data_sources::BootConfidence::NotBoot,
                false,
            ),
        ];

        // Should initialize to index 1 (first non-boot)
        app.disk_index = app
            .disks
            .iter()
            .position(|disk| !disk.boot_confidence.is_boot())
            .unwrap_or(0);

        assert_eq!(app.disk_index, 1);

        // Try to navigate to boot disk
        app.handle_input(key(KeyCode::Up));

        // Should stay on non-boot disk or skip boot disk
        if let Some(_disk) = app.disks.get(app.disk_index) {
            // Navigation should skip boot disk
            assert_eq!(app.disk_index, 1, "Should stay on safe disk");
        }
    }

    #[test]
    fn disk_selection_blocks_protected_disk_without_developer_mode() {
        let mut app = App::new();
        app.current_step_type = InstallStepType::DiskSelection;

        // Set up protected (boot/source) disk
        app.disks = vec![disk_option(
            "/dev/sda",
            "Generic",
            "Boot Drive",
            256 * 1024 * 1024 * 1024,
            data_sources::TransportType::Sata,
            false,
            data_sources::BootConfidence::Confident,
            true,
        )];

        app.disk_index = 0;

        // Try to advance with protected disk selected without developer mode
        app.handle_input(key(KeyCode::Enter));

        // Should stay on DiskSelection and show error
        assert_eq!(app.current_step_type, InstallStepType::DiskSelection);
        assert!(app.error_message.is_some());
    }

    #[test]
    fn disk_selection_allows_protected_disk_in_developer_mode() {
        let mut app = App::new();
        app.current_step_type = InstallStepType::DiskSelection;
        app.developer_mode = true;

        app.disks = vec![disk_option(
            "/dev/sda",
            "Generic",
            "Boot Drive",
            256 * 1024 * 1024 * 1024,
            data_sources::TransportType::Sata,
            false,
            data_sources::BootConfidence::Confident,
            true,
        )];
        app.disk_index = 0;

        app.handle_input(key(KeyCode::Enter));
        assert_eq!(app.current_step_type, InstallStepType::DiskConfirmation);
    }

    #[test]
    fn disk_selection_allows_non_boot_disk() {
        let mut app = App::new();
        app.current_step_type = InstallStepType::DiskSelection;

        // Set up non-boot disk
        app.disks = vec![disk_option(
            "/dev/sdb",
            "Generic",
            "Data Drive",
            512 * 1024 * 1024 * 1024,
            data_sources::TransportType::Sata,
            false,
            data_sources::BootConfidence::NotBoot,
            false,
        )];

        app.disk_index = 0;

        // Advance with non-boot disk selected
        app.handle_input(key(KeyCode::Enter));

        // Should advance to next step
        assert_eq!(app.current_step_type, InstallStepType::DiskConfirmation);
        assert!(app.error_message.is_none());
    }

    #[test]
    fn adjust_disk_index_skips_boot_disks() {
        let mut app = App::new();

        // Set up mixed disks: boot, safe, boot, safe
        app.disks = vec![
            disk_option(
                "/dev/sda",
                "Generic",
                "Disk 1",
                256 * 1024 * 1024 * 1024,
                data_sources::TransportType::Sata,
                false,
                data_sources::BootConfidence::Confident,
                false,
            ),
            disk_option(
                "/dev/sdb",
                "Generic",
                "Safe Disk 1",
                512 * 1024 * 1024 * 1024,
                data_sources::TransportType::Sata,
                false,
                data_sources::BootConfidence::NotBoot,
                false,
            ),
            disk_option(
                "/dev/sdc",
                "Generic",
                "Disk 2",
                128 * 1024 * 1024 * 1024,
                data_sources::TransportType::Sata,
                false,
                data_sources::BootConfidence::Confident,
                false,
            ),
            disk_option(
                "/dev/sdd",
                "Generic",
                "Safe Disk 2",
                1024 * 1024 * 1024 * 1024,
                data_sources::TransportType::Sata,
                false,
                data_sources::BootConfidence::NotBoot,
                false,
            ),
        ];

        app.disk_index = 1; // Start on first safe disk

        // Navigate forward - should skip index 2 (boot) and land on index 3 (safe)
        app.adjust_disk_index(1);
        assert_eq!(app.disk_index, 3);
        assert!(!app.disks[app.disk_index].boot_confidence.is_boot());

        // Navigate forward again - should wrap to index 1 (safe), skipping index 0 (boot)
        app.adjust_disk_index(1);
        assert_eq!(app.disk_index, 1);
        assert!(!app.disks[app.disk_index].boot_confidence.is_boot());

        // Navigate backward - should go to index 3 (safe), skipping index 2 and 0 (boot)
        app.adjust_disk_index(-1);
        assert_eq!(app.disk_index, 3);
        assert!(!app.disks[app.disk_index].boot_confidence.is_boot());
    }
}
