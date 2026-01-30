//! Application state machine for the TUI wizard
#![allow(dead_code)]
#![allow(clippy::collapsible_else_if)]

use crate::cli::{Cli, PartitionScheme};
use crate::locale::LocaleConfig;
use crossterm::event::{KeyCode, KeyEvent};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, Sender};

use super::input::{InputField, InputMode};
use super::progress::{Phase, ProgressState};
use super::widgets::DiskInfo;
use super::flash_config::{FlashConfig, ImageSource, ImageVersionOption, ImageEditionOption};

// ============================================================================
// Configuration Steps (user input phase)
// ============================================================================

/// Configuration steps requiring user input (before installation)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ConfigStep {
    DiskSelection,
    DiskConfirmation,
    PartitionScheme,
    PartitionLayout,
    PartitionCustomize,
    ImageSource,
    ImageSelection,
    UefiSource,
    LocaleSelection,
    Options,
    FinalSummary,
}

impl ConfigStep {
    pub fn title(&self) -> &'static str {
        match self {
            ConfigStep::DiskSelection => "Select Disk",
            ConfigStep::DiskConfirmation => "Confirm Disk",
            ConfigStep::PartitionScheme => "Partition Scheme",
            ConfigStep::PartitionLayout => "Partition Layout",
            ConfigStep::PartitionCustomize => "Customize Partitions",
            ConfigStep::ImageSource => "Image Source",
            ConfigStep::ImageSelection => "Select Image",
            ConfigStep::UefiSource => "UEFI Source",
            ConfigStep::LocaleSelection => "Locale & Keymap",
            ConfigStep::Options => "Options",
            ConfigStep::FinalSummary => "Final Summary",
        }
    }

    pub fn all() -> &'static [ConfigStep] {
        &[
            ConfigStep::DiskSelection,
            ConfigStep::DiskConfirmation,
            ConfigStep::PartitionScheme,
            ConfigStep::PartitionLayout,
            ConfigStep::PartitionCustomize,
            ConfigStep::ImageSource,
            ConfigStep::ImageSelection,
            ConfigStep::UefiSource,
            ConfigStep::LocaleSelection,
            ConfigStep::Options,
            ConfigStep::FinalSummary,
        ]
    }
}

// ============================================================================
// Execution Steps (installation phase)
// ============================================================================

/// Execution phases (maps to flash.rs Phase enum)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ExecutionStep {
    DownloadImage,
    DownloadUefi,
    Partition,
    Format,
    CopyRoot,
    CopyBoot,
    CopyEfi,
    UefiConfig,
    LocaleConfig,
    Fstab,
    StageDojo,
    Cleanup,
}

impl ExecutionStep {
    pub fn title(&self) -> &'static str {
        match self {
            ExecutionStep::DownloadImage => "Download Image",
            ExecutionStep::DownloadUefi => "Download UEFI",
            ExecutionStep::Partition => "Partition Disk",
            ExecutionStep::Format => "Format Partitions",
            ExecutionStep::CopyRoot => "Copy Root",
            ExecutionStep::CopyBoot => "Copy Boot",
            ExecutionStep::CopyEfi => "Copy EFI",
            ExecutionStep::UefiConfig => "Configure UEFI",
            ExecutionStep::LocaleConfig => "Configure Locale",
            ExecutionStep::Fstab => "Generate fstab",
            ExecutionStep::StageDojo => "Stage Dojo",
            ExecutionStep::Cleanup => "Cleanup",
        }
    }

    pub fn all() -> &'static [ExecutionStep] {
        &[
            ExecutionStep::DownloadImage,
            ExecutionStep::DownloadUefi,
            ExecutionStep::Partition,
            ExecutionStep::Format,
            ExecutionStep::CopyRoot,
            ExecutionStep::CopyBoot,
            ExecutionStep::CopyEfi,
            ExecutionStep::UefiConfig,
            ExecutionStep::LocaleConfig,
            ExecutionStep::Fstab,
            ExecutionStep::StageDojo,
            ExecutionStep::Cleanup,
        ]
    }

    /// Map from flash.rs Phase to ExecutionStep
    pub fn from_phase(phase: Phase) -> Self {
        match phase {
            Phase::Partition => ExecutionStep::Partition,
            Phase::Format => ExecutionStep::Format,
            Phase::CopyRoot => ExecutionStep::CopyRoot,
            Phase::CopyBoot => ExecutionStep::CopyBoot,
            Phase::CopyEfi => ExecutionStep::CopyEfi,
            Phase::UefiConfig => ExecutionStep::UefiConfig,
            Phase::LocaleConfig => ExecutionStep::LocaleConfig,
            Phase::Fstab => ExecutionStep::Fstab,
            Phase::StageDojo => ExecutionStep::StageDojo,
            Phase::Cleanup => ExecutionStep::Cleanup,
        }
    }
}

// ============================================================================
// Step State
// ============================================================================

/// State of any step
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum StepState {
    #[default]
    Pending,
    Current,
    Completed,
    Skipped,
    Failed,
}

impl StepState {
    pub fn symbol(&self) -> &'static str {
        match self {
            StepState::Pending => "  ",
            StepState::Current => ">>",
            StepState::Completed => "[OK]",
            StepState::Skipped => "--",
            StepState::Failed => "[!]",
        }
    }
}

// ============================================================================
// Partition Size Types
// ============================================================================

/// Strongly-typed partition size
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PartitionSize {
    Mebibytes(u64),
    Gibibytes(u64),
    Percentage(u8),
}

impl PartitionSize {
    pub fn to_parted_string(&self) -> String {
        match self {
            PartitionSize::Mebibytes(m) => format!("{}MiB", m),
            PartitionSize::Gibibytes(g) => format!("{}GiB", g),
            PartitionSize::Percentage(p) => format!("{}%", p),
        }
    }

    pub fn parse(s: &str) -> Result<Self, String> {
        let s = s.trim().to_uppercase();
        if s.ends_with('%') {
            let val: u8 = s
                .trim_end_matches('%')
                .parse()
                .map_err(|_| "Invalid percentage".to_string())?;
            return Ok(PartitionSize::Percentage(val));
        }
        if s.contains("GIB") || s.contains("GB") || s.ends_with('G') {
            let val: u64 = s
                .trim_end_matches("GIB")
                .trim_end_matches("GB")
                .trim_end_matches('G')
                .parse()
                .map_err(|_| "Invalid GiB value".to_string())?;
            return Ok(PartitionSize::Gibibytes(val));
        }
        if s.contains("MIB") || s.contains("MB") || s.ends_with('M') {
            let val: u64 = s
                .trim_end_matches("MIB")
                .trim_end_matches("MB")
                .trim_end_matches('M')
                .parse()
                .map_err(|_| "Invalid MiB value".to_string())?;
            return Ok(PartitionSize::Mebibytes(val));
        }
        Err("Size must end with M, G, MiB, GiB, or %".to_string())
    }

    pub fn display(&self) -> String {
        match self {
            PartitionSize::Mebibytes(m) => {
                if *m >= 1024 && m % 1024 == 0 {
                    format!("{}G", m / 1024)
                } else {
                    format!("{}M", m)
                }
            }
            PartitionSize::Gibibytes(g) => format!("{}G", g),
            PartitionSize::Percentage(p) => format!("{}%", p),
        }
    }
}

// ============================================================================
// Partition Plan
// ============================================================================

/// Partition layout configuration
#[derive(Debug, Clone)]
pub struct PartitionPlan {
    pub scheme: PartitionScheme,
    pub efi_size: PartitionSize,
    pub boot_size: PartitionSize,
    pub root_end: PartitionSize,
    pub use_recommended: bool,
}

impl Default for PartitionPlan {
    fn default() -> Self {
        Self {
            scheme: PartitionScheme::Mbr,
            efi_size: PartitionSize::Mebibytes(1024),
            boot_size: PartitionSize::Mebibytes(2048),
            root_end: PartitionSize::Gibibytes(1800),
            use_recommended: true,
        }
    }
}

// ============================================================================
// Installation Mode and State
// ============================================================================

/// Current installation mode
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstallMode {
    Welcome,
    Configuring,
    Executing,
    Complete,
}

/// Unified installation state for single-page view
#[derive(Debug, Clone)]
pub struct InstallationState {
    pub mode: InstallMode,
    pub config_states: HashMap<ConfigStep, StepState>,
    pub exec_states: HashMap<ExecutionStep, StepState>,
    pub current_config: Option<ConfigStep>,
    pub current_exec: Option<ExecutionStep>,
    pub overall_percent: f64,
    pub status_message: String,
    pub error: Option<String>,
}

impl Default for InstallationState {
    fn default() -> Self {
        let mut config_states = HashMap::new();
        for step in ConfigStep::all() {
            config_states.insert(*step, StepState::Pending);
        }
        let mut exec_states = HashMap::new();
        for step in ExecutionStep::all() {
            exec_states.insert(*step, StepState::Pending);
        }
        Self {
            mode: InstallMode::Welcome,
            config_states,
            exec_states,
            current_config: None,
            current_exec: None,
            overall_percent: 0.0,
            status_message: "Ready to begin".to_string(),
            error: None,
        }
    }
}

// ============================================================================
// Legacy InstallStep (retained for backward compatibility during transition)
// ============================================================================

/// Defines the sub-tasks or steps within the main installation wizard screen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstallStep {
    Welcome,
    DiskSelection,
    DiskConfirmation,
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

impl InstallStep {
    pub fn title(&self) -> &'static str {
        match self {
            InstallStep::Welcome => "Enter the Dojo",
            InstallStep::DiskSelection => "Select Target Disk",
            InstallStep::DiskConfirmation => "Confirm Disk Destruction",
            InstallStep::PartitionScheme => "Partition Scheme",
            InstallStep::PartitionLayout => "Partition Layout",
            InstallStep::PartitionCustomize => "Customize Partitions",
            InstallStep::DownloadSourceSelection => "Select Image Source",
            InstallStep::ImageSelection => "Select Image File",
            InstallStep::UefiDirectory => "UEFI Configuration",
            InstallStep::LocaleSelection => "Locale & Keymap",
            InstallStep::Options => "Installation Options",
            InstallStep::Confirmation => "Final Confirmation",
            InstallStep::DownloadingFedora => "Downloading Fedora Image",
            InstallStep::DownloadingUefi => "Downloading UEFI Firmware",
            InstallStep::Flashing => "Installing...",
            InstallStep::Complete => "Installation Complete!",
        }
    }

    // Helper to get the next step in the sequence
    pub fn next(&self) -> Option<InstallStep> {
        match self {
            InstallStep::Welcome => Some(InstallStep::DiskSelection),
            InstallStep::DiskSelection => Some(InstallStep::DiskConfirmation),
            InstallStep::DiskConfirmation => Some(InstallStep::PartitionScheme),
            InstallStep::PartitionScheme => Some(InstallStep::PartitionLayout),
            InstallStep::PartitionLayout => Some(InstallStep::DownloadSourceSelection),
            InstallStep::PartitionCustomize => Some(InstallStep::DownloadSourceSelection),
            InstallStep::DownloadSourceSelection => Some(InstallStep::ImageSelection),
            InstallStep::ImageSelection => Some(InstallStep::UefiDirectory),
            InstallStep::UefiDirectory => Some(InstallStep::LocaleSelection),
            InstallStep::LocaleSelection => Some(InstallStep::Options),
            InstallStep::Options => Some(InstallStep::Confirmation),
            InstallStep::Confirmation => None,
            InstallStep::DownloadingFedora => None,
            InstallStep::DownloadingUefi => None,
            InstallStep::Flashing => Some(InstallStep::Complete),
            InstallStep::Complete => None,
        }
    }

    // Helper to get the previous step in the sequence
    pub fn prev(&self) -> Option<InstallStep> {
        match self {
            InstallStep::Welcome => None,
            InstallStep::DiskSelection => Some(InstallStep::Welcome),
            InstallStep::DiskConfirmation => Some(InstallStep::DiskSelection),
            InstallStep::PartitionScheme => Some(InstallStep::DiskConfirmation),
            InstallStep::PartitionLayout => Some(InstallStep::PartitionScheme),
            InstallStep::PartitionCustomize => Some(InstallStep::PartitionLayout),
            InstallStep::DownloadSourceSelection => Some(InstallStep::PartitionLayout),
            InstallStep::ImageSelection => Some(InstallStep::DownloadSourceSelection),
            InstallStep::UefiDirectory => Some(InstallStep::ImageSelection),
            InstallStep::LocaleSelection => Some(InstallStep::UefiDirectory),
            InstallStep::Options => Some(InstallStep::LocaleSelection),
            InstallStep::Confirmation => Some(InstallStep::Options),
            _ => None,
        }
    }

    /// Check if this step is part of the configuration phase
    pub fn is_config_step(&self) -> bool {
        matches!(
            self,
            InstallStep::DiskSelection
                | InstallStep::DiskConfirmation
                | InstallStep::PartitionScheme
                | InstallStep::PartitionLayout
                | InstallStep::PartitionCustomize
                | InstallStep::DownloadSourceSelection
                | InstallStep::ImageSelection
                | InstallStep::UefiDirectory
                | InstallStep::LocaleSelection
                | InstallStep::Options
                | InstallStep::Confirmation
        )
    }

    /// Check if this step is part of the execution phase
    pub fn is_exec_step(&self) -> bool {
        matches!(
            self,
            InstallStep::DownloadingFedora | InstallStep::DownloadingUefi | InstallStep::Flashing
        )
    }
}







/// Result of handling input
pub enum InputResult {
    Continue,
    Quit,
    Complete,
    StartFlash(FlashConfig),     // New: to signal main.rs to start flashing
    StartDownload(DownloadType), // New: to signal main.rs to start a download
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

/// Installation options (checkboxes)
#[derive(Debug, Clone)]
pub struct InstallOptions {
    pub auto_unmount: bool,
    pub early_ssh: bool,
    pub partition_scheme: PartitionScheme,
    pub dry_run: bool,
}

impl Default for InstallOptions {
    fn default() -> Self {
        Self {
            auto_unmount: true,
            early_ssh: true, // Default ON as per spec
            partition_scheme: PartitionScheme::Mbr,
            dry_run: false,
        }
    }
}

/// Download progress state for TUI display
#[derive(Debug, Clone, Default)]
pub struct DownloadState {
    pub is_downloading: bool,
    pub is_complete: bool,
    pub current_bytes: u64,
    pub total_bytes: Option<u64>,
    pub speed_bytes_per_sec: u64,
    pub eta_seconds: u64,
    pub description: String,
    pub error: Option<String>,
    pub phase: DownloadPhase,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum DownloadPhase {
    #[default]
    NotStarted,
    Downloading,
    Extracting,
    Complete,
    Failed,
}

/// Progress update sent from download thread
#[derive(Debug, Clone)]
pub enum DownloadUpdate {
    Started {
        description: String,
        total_bytes: Option<u64>,
    },
    Progress {
        current_bytes: u64,
        speed: u64,
        eta: u64,
    },
    Extracting,
    Complete(PathBuf), // Include downloaded path
    Error(String),
}



/// Application state
pub struct App {
    pub current_step: InstallStep,
    pub installation_state: InstallationState,
    pub mash_root: PathBuf,
    pub watch: bool,
    pub dry_run_cli: bool,

    // Animation state
    pub animation_tick: u64,

    // Disk selection
    pub available_disks: Vec<DiskInfo>,
    pub selected_disk_index: usize,

    // Disk confirmation (type "DESTROY")
    pub disk_confirm_input: String,
    pub disk_confirm_error: Option<String>,

    // Partition configuration
    pub partition_plan: PartitionPlan,
    pub partition_scheme_focus: usize,
    pub partition_edit_field: usize,
    pub partition_edit_input: String,
    pub partition_edit_error: Option<String>,

    // Image source selection
    pub image_source_selection: ImageSource,
    pub selected_image_source_index: usize,
    pub selected_image_version_index: usize,
    pub selected_image_edition_index: usize,
    pub download_uefi_firmware: bool,

    // Image selection (local file path)
    pub image_input: InputField,
    pub image_error: Option<String>,

    // UEFI directory
    pub uefi_input: InputField,
    pub uefi_error: Option<String>,

    // Locale selection
    pub available_locales: Vec<LocaleConfig>,
    pub selected_locale_index: usize,

    // Options
    pub options: InstallOptions,
    pub options_focus: usize,

    // Confirmation (type "FLASH")
    pub confirmation_input: String,
    pub confirmation_error: Option<String>,

    // Download progress
    pub download_state: DownloadState,
    pub download_rx: Option<Receiver<DownloadUpdate>>,
    pub downloaded_image_path: Option<PathBuf>,
    pub downloaded_uefi_path: Option<PathBuf>,

    // Progress
    pub progress: ProgressState,
    pub progress_rx: Option<Receiver<super::progress::ProgressUpdate>>,

    // Complete
    pub install_success: bool,
    pub install_error: Option<String>,

    // Partition sizes from CLI (used as defaults for TUI if not overridden)
    pub efi_size_cli: String,
    pub boot_size_cli: String,
    pub root_end_cli: String,
}

impl App {
    pub fn new(cli: &Cli, watch: bool, dry_run: bool) -> Self {
        let mash_root = cli.mash_root.clone();

        // Default paths
        let default_image_dir = mash_root.join("images");
        let default_uefi_dir = mash_root.join("uefi");

        // Scan for available disks
        let available_disks = DiskInfo::scan_disks();

        // Available locales
        let available_locales = crate::locale::LOCALES.to_vec();

        // Parse CLI partition sizes into PartitionPlan
        let efi_size_cli = match &cli.command {
            Some(crate::cli::Command::Flash { efi_size, .. }) => efi_size.clone(),
            _ => "1024MiB".to_string(),
        };
        let boot_size_cli = match &cli.command {
            Some(crate::cli::Command::Flash { boot_size, .. }) => boot_size.clone(),
            _ => "2048MiB".to_string(),
        };
        let root_end_cli = match &cli.command {
            Some(crate::cli::Command::Flash { root_end, .. }) => root_end.clone(),
            _ => "1800GiB".to_string(),
        };

        let partition_plan = PartitionPlan {
            scheme: PartitionScheme::Mbr,
            efi_size: PartitionSize::parse(&efi_size_cli).unwrap_or(PartitionSize::Mebibytes(1024)),
            boot_size: PartitionSize::parse(&boot_size_cli)
                .unwrap_or(PartitionSize::Mebibytes(2048)),
            root_end: PartitionSize::parse(&root_end_cli).unwrap_or(PartitionSize::Gibibytes(1800)),
            use_recommended: true,
        };

        Self {
            current_step: InstallStep::Welcome,
            installation_state: InstallationState::default(),
            mash_root,
            watch,
            dry_run_cli: dry_run,

            animation_tick: 0,

            available_disks,
            selected_disk_index: 0,

            // Disk confirmation
            disk_confirm_input: String::new(),
            disk_confirm_error: None,

            // Partition configuration
            partition_plan,
            partition_scheme_focus: 0,
            partition_edit_field: 0,
            partition_edit_input: String::new(),
            partition_edit_error: None,

            // Image source selection
            image_source_selection: ImageSource::LocalFile,
            selected_image_source_index: 0,
            selected_image_version_index: 0,
            selected_image_edition_index: 0,
            download_uefi_firmware: false,

            image_input: InputField::new(
                default_image_dir.to_string_lossy().to_string(),
                "Path to Fedora .raw image",
            ),
            image_error: None,

            uefi_input: InputField::new(
                default_uefi_dir.to_string_lossy().to_string(),
                "UEFI overlay directory",
            ),
            uefi_error: None,

            available_locales,
            selected_locale_index: 0,

            options: InstallOptions {
                dry_run,
                ..Default::default()
            },
            options_focus: 0,

            confirmation_input: String::new(),
            confirmation_error: None,

            download_state: DownloadState::default(),
            download_rx: None,
            downloaded_image_path: None,
            downloaded_uefi_path: None,

            progress: ProgressState::default(),
            progress_rx: None,

            install_success: false,
            install_error: None,

            efi_size_cli,
            boot_size_cli,
            root_end_cli,
        }
    }

    /// Handle keyboard input, returns action to take
    pub fn handle_input(&mut self, key: KeyEvent) -> InputResult {
        match self.current_step {
            InstallStep::Welcome => self.handle_welcome_input(key),
            InstallStep::DiskSelection => self.handle_disk_selection_input(key),
            InstallStep::DiskConfirmation => self.handle_disk_confirmation_input(key),
            InstallStep::PartitionScheme => self.handle_partition_scheme_input(key),
            InstallStep::PartitionLayout => self.handle_partition_layout_input(key),
            InstallStep::PartitionCustomize => self.handle_partition_customize_input(key),
            InstallStep::DownloadSourceSelection => {
                self.handle_download_source_selection_input(key)
            }
            InstallStep::ImageSelection => self.handle_image_selection_input(key),
            InstallStep::UefiDirectory => self.handle_uefi_input(key),
            InstallStep::LocaleSelection => self.handle_locale_selection_input(key),
            InstallStep::Options => self.handle_options_input(key),
            InstallStep::Confirmation => self.handle_confirmation_input(key),
            InstallStep::DownloadingFedora | InstallStep::DownloadingUefi => {
                self.handle_download_input(key)
            }
            InstallStep::Flashing => self.handle_flashing_input(key),
            InstallStep::Complete => self.handle_complete_input(key),
        }
    }

    fn handle_welcome_input(&mut self, key: KeyEvent) -> InputResult {
        match key.code {
            KeyCode::Enter => {
                self.current_step = InstallStep::DiskSelection;
                InputResult::Continue
            }
            KeyCode::Esc | KeyCode::Char('q') => InputResult::Quit,
            _ => InputResult::Continue,
        }
    }

    fn handle_disk_selection_input(&mut self, key: KeyEvent) -> InputResult {
        match key.code {
            KeyCode::Up | KeyCode::Char('k') => {
                if self.selected_disk_index > 0 {
                    self.selected_disk_index -= 1;
                }
                InputResult::Continue
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if self.selected_disk_index < self.available_disks.len().saturating_sub(1) {
                    self.selected_disk_index += 1;
                }
                InputResult::Continue
            }
            KeyCode::Enter => {
                if !self.available_disks.is_empty() {
                    self.current_step = InstallStep::DiskConfirmation;
                    self.disk_confirm_input.clear();
                    self.disk_confirm_error = None;
                }
                InputResult::Continue
            }
            KeyCode::Esc => {
                if let Some(prev) = self.current_step.prev() {
                    self.current_step = prev;
                }
                InputResult::Continue
            }
            KeyCode::Char('r') => {
                self.available_disks = DiskInfo::scan_disks();
                self.selected_disk_index = 0;
                InputResult::Continue
            }
            _ => InputResult::Continue,
        }
    }

    fn handle_disk_confirmation_input(&mut self, key: KeyEvent) -> InputResult {
        match key.code {
            KeyCode::Char(c) => {
                self.disk_confirm_input.push(c);
                self.disk_confirm_error = None;
                InputResult::Continue
            }
            KeyCode::Backspace => {
                self.disk_confirm_input.pop();
                self.disk_confirm_error = None;
                InputResult::Continue
            }
            KeyCode::Enter => {
                if self.disk_confirm_input.trim() == "DESTROY" {
                    self.disk_confirm_error = None;
                    self.current_step = InstallStep::PartitionScheme;
                } else {
                    self.disk_confirm_error =
                        Some("Type exactly: DESTROY (case sensitive)".to_string());
                }
                InputResult::Continue
            }
            KeyCode::Esc => {
                self.current_step = InstallStep::DiskSelection;
                self.disk_confirm_input.clear();
                self.disk_confirm_error = None;
                InputResult::Continue
            }
            _ => InputResult::Continue,
        }
    }

    fn handle_partition_scheme_input(&mut self, key: KeyEvent) -> InputResult {
        match key.code {
            KeyCode::Up | KeyCode::Char('k') => {
                if self.partition_scheme_focus > 0 {
                    self.partition_scheme_focus -= 1;
                }
                InputResult::Continue
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if self.partition_scheme_focus < 1 {
                    self.partition_scheme_focus += 1;
                }
                InputResult::Continue
            }
            KeyCode::Enter | KeyCode::Char(' ') => {
                self.partition_plan.scheme = match self.partition_scheme_focus {
                    0 => PartitionScheme::Mbr,
                    _ => PartitionScheme::Gpt,
                };
                self.current_step = InstallStep::PartitionLayout;
                InputResult::Continue
            }
            KeyCode::Esc => {
                if let Some(prev) = self.current_step.prev() {
                    self.current_step = prev;
                }
                InputResult::Continue
            }
            _ => InputResult::Continue,
        }
    }

    fn handle_partition_layout_input(&mut self, key: KeyEvent) -> InputResult {
        match key.code {
            KeyCode::Char('y') | KeyCode::Char('Y') | KeyCode::Enter => {
                self.partition_plan.use_recommended = true;
                self.current_step = InstallStep::DownloadSourceSelection;
                InputResult::Continue
            }
            KeyCode::Char('n') | KeyCode::Char('N') => {
                self.partition_plan.use_recommended = false;
                self.current_step = InstallStep::PartitionCustomize;
                self.partition_edit_field = 0;
                self.partition_edit_input = self.partition_plan.efi_size.display();
                InputResult::Continue
            }
            KeyCode::Esc => {
                if let Some(prev) = self.current_step.prev() {
                    self.current_step = prev;
                }
                InputResult::Continue
            }
            _ => InputResult::Continue,
        }
    }

    fn handle_partition_customize_input(&mut self, key: KeyEvent) -> InputResult {
        match key.code {
            KeyCode::Tab | KeyCode::Down => {
                // Save current field
                self.save_partition_field();
                // Move to next field
                self.partition_edit_field = (self.partition_edit_field + 1) % 3;
                self.load_partition_field();
                InputResult::Continue
            }
            KeyCode::BackTab | KeyCode::Up => {
                // Save current field
                self.save_partition_field();
                // Move to previous field
                self.partition_edit_field = if self.partition_edit_field == 0 {
                    2
                } else {
                    self.partition_edit_field - 1
                };
                self.load_partition_field();
                InputResult::Continue
            }
            KeyCode::Enter => {
                self.save_partition_field();
                if self.partition_edit_error.is_none() {
                    self.current_step = InstallStep::DownloadSourceSelection;
                }
                InputResult::Continue
            }
            KeyCode::Char(c) => {
                self.partition_edit_input.push(c);
                self.partition_edit_error = None;
                InputResult::Continue
            }
            KeyCode::Backspace => {
                self.partition_edit_input.pop();
                self.partition_edit_error = None;
                InputResult::Continue
            }
            KeyCode::Esc => {
                if let Some(prev) = self.current_step.prev() {
                    self.current_step = prev;
                }
                InputResult::Continue
            }
            _ => InputResult::Continue,
        }
    }

    fn save_partition_field(&mut self) {
        let result = PartitionSize::parse(&self.partition_edit_input);
        match result {
            Ok(size) => {
                match self.partition_edit_field {
                    0 => self.partition_plan.efi_size = size,
                    1 => self.partition_plan.boot_size = size,
                    2 => self.partition_plan.root_end = size,
                    _ => {}
                }
                self.partition_edit_error = None;
            }
            Err(e) => {
                self.partition_edit_error = Some(e);
            }
        }
    }

    fn load_partition_field(&mut self) {
        self.partition_edit_input = match self.partition_edit_field {
            0 => self.partition_plan.efi_size.display(),
            1 => self.partition_plan.boot_size.display(),
            2 => self.partition_plan.root_end.display(),
            _ => String::new(),
        };
        self.partition_edit_error = None;
    }

    // New handler for download source selection
    fn handle_download_source_selection_input(&mut self, key: KeyEvent) -> InputResult {
        let max_source_index = 1; // 0: LocalFile, 1: DownloadFedora
        match key.code {
            KeyCode::Up | KeyCode::Char('k') => {
                if self.selected_image_source_index > 0 {
                    self.selected_image_source_index -= 1;
                }
                InputResult::Continue
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if self.selected_image_source_index < max_source_index {
                    self.selected_image_source_index += 1;
                }
                InputResult::Continue
            }
            KeyCode::Enter => {
                self.image_source_selection = match self.selected_image_source_index {
                    0 => ImageSource::LocalFile,
                    1 => ImageSource::DownloadFedora,
                    _ => unreachable!(),
                };
                self.current_step = InstallStep::ImageSelection;
                InputResult::Continue
            }
            KeyCode::Esc => {
                if let Some(prev) = self.current_step.prev() {
                    self.current_step = prev;
                }
                InputResult::Continue
            }
            _ => InputResult::Continue,
        }
    }

    fn handle_image_selection_input(&mut self, key: KeyEvent) -> InputResult {
        match self.image_source_selection {
            ImageSource::LocalFile => {
                if self.image_input.mode == InputMode::Editing {
                    match key.code {
                        KeyCode::Enter => {
                            let path = PathBuf::from(self.image_input.value());
                            if path.exists() && path.is_file() {
                                self.image_error = None;
                                self.image_input.mode = InputMode::Normal;
                                self.current_step = InstallStep::UefiDirectory;
                            } else if path.is_dir() {
                                self.image_error =
                                    Some("Please select a .raw file, not a directory".into());
                            } else {
                                self.image_error =
                                    Some(format!("File not found: {}", path.display()));
                            }
                            InputResult::Continue
                        }
                        KeyCode::Esc => {
                            self.image_input.mode = InputMode::Normal;
                            InputResult::Continue
                        }
                        _ => {
                            self.image_input.handle_key(key);
                            self.image_error = None;
                            InputResult::Continue
                        }
                    }
                } else {
                    match key.code {
                        KeyCode::Enter | KeyCode::Char('e') | KeyCode::Char('i') => {
                            self.image_input.mode = InputMode::Editing;
                            InputResult::Continue
                        }
                        KeyCode::Esc => {
                            if let Some(prev) = self.current_step.prev() {
                                self.current_step = prev;
                            }
                            InputResult::Continue
                        }
                        KeyCode::Tab => {
                            self.current_step = InstallStep::UefiDirectory;
                            InputResult::Continue
                        }
                        _ => InputResult::Continue,
                    }
                }
            }
            ImageSource::DownloadFedora => {
                let max_version_index = ImageVersionOption::all().len().saturating_sub(1);
                let max_edition_index = ImageEditionOption::all().len().saturating_sub(1);

                match key.code {
                    KeyCode::Up | KeyCode::Char('k') => {
                        if self.options_focus == 0 {
                            if self.selected_image_version_index > 0 {
                                self.selected_image_version_index -= 1;
                            }
                        } else if self.selected_image_edition_index > 0 {
                            self.selected_image_edition_index -= 1;
                        }
                        InputResult::Continue
                    }
                    KeyCode::Down | KeyCode::Char('j') => {
                        if self.options_focus == 0 {
                            if self.selected_image_version_index < max_version_index {
                                self.selected_image_version_index += 1;
                            }
                        } else {
                            if self.selected_image_edition_index < max_edition_index {
                                self.selected_image_edition_index += 1;
                            }
                        }
                        InputResult::Continue
                    }
                    KeyCode::Left | KeyCode::Right => {
                        self.options_focus = (self.options_focus + 1) % 2;
                        InputResult::Continue
                    }
                    KeyCode::Enter | KeyCode::Tab => {
                        self.current_step = InstallStep::UefiDirectory;
                        InputResult::Continue
                    }
                    KeyCode::Esc => {
                        if let Some(prev) = self.current_step.prev() {
                            self.current_step = prev;
                        }
                        InputResult::Continue
                    }
                    _ => InputResult::Continue,
                }
            }
        }
    }

    fn handle_uefi_input(&mut self, key: KeyEvent) -> InputResult {
        if self.uefi_input.mode == InputMode::Editing {
            match key.code {
                KeyCode::Enter => {
                    let path = PathBuf::from(self.uefi_input.value());
                    if path.exists() && path.is_dir() {
                        self.uefi_error = None;
                        self.uefi_input.mode = InputMode::Normal;
                        self.current_step = InstallStep::LocaleSelection;
                    } else {
                        self.uefi_error = Some(format!("Directory not found: {}", path.display()));
                    }
                    InputResult::Continue
                }
                KeyCode::Esc => {
                    self.uefi_input.mode = InputMode::Normal;
                    InputResult::Continue
                }
                _ => {
                    self.uefi_input.handle_key(key);
                    self.uefi_error = None;
                    InputResult::Continue
                }
            }
        } else {
            match key.code {
                KeyCode::Char('d') => {
                    self.download_uefi_firmware = !self.download_uefi_firmware;
                    InputResult::Continue
                }
                KeyCode::Enter | KeyCode::Char('e') | KeyCode::Char('i') => {
                    self.uefi_input.mode = InputMode::Editing;
                    InputResult::Continue
                }
                KeyCode::Esc => {
                    if let Some(prev) = self.current_step.prev() {
                        self.current_step = prev;
                    }
                    InputResult::Continue
                }
                KeyCode::Tab => {
                    self.current_step = InstallStep::LocaleSelection;
                    InputResult::Continue
                }
                _ => InputResult::Continue,
            }
        }
    }

    fn handle_locale_selection_input(&mut self, key: KeyEvent) -> InputResult {
        match key.code {
            KeyCode::Up | KeyCode::Char('k') => {
                if self.selected_locale_index > 0 {
                    self.selected_locale_index -= 1;
                }
                InputResult::Continue
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if self.selected_locale_index < self.available_locales.len().saturating_sub(1) {
                    self.selected_locale_index += 1;
                }
                InputResult::Continue
            }
            KeyCode::Enter | KeyCode::Tab => {
                self.current_step = InstallStep::Options;
                InputResult::Continue
            }
            KeyCode::Esc => {
                if let Some(prev) = self.current_step.prev() {
                    self.current_step = prev;
                }
                InputResult::Continue
            }
            _ => InputResult::Continue,
        }
    }

    fn handle_options_input(&mut self, key: KeyEvent) -> InputResult {
        match key.code {
            KeyCode::Up | KeyCode::Char('k') => {
                if self.options_focus > 0 {
                    self.options_focus -= 1;
                }
                InputResult::Continue
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if self.options_focus < 3 {
                    self.options_focus += 1;
                }
                InputResult::Continue
            }
            KeyCode::Char(' ') | KeyCode::Enter => {
                match self.options_focus {
                    0 => self.options.auto_unmount = !self.options.auto_unmount,
                    1 => self.options.early_ssh = !self.options.early_ssh,
                    2 => {
                        self.options.partition_scheme = match self.options.partition_scheme {
                            PartitionScheme::Mbr => PartitionScheme::Gpt,
                            PartitionScheme::Gpt => PartitionScheme::Mbr,
                        };
                    }
                    3 => self.options.dry_run = !self.options.dry_run,
                    _ => {}
                }
                InputResult::Continue
            }
            KeyCode::Tab => {
                self.current_step = InstallStep::Confirmation;
                InputResult::Continue
            }
            KeyCode::Esc => {
                if let Some(prev) = self.current_step.prev() {
                    self.current_step = prev;
                }
                InputResult::Continue
            }
            _ => InputResult::Continue,
        }
    }

    fn handle_confirmation_input(&mut self, key: KeyEvent) -> InputResult {
        match key.code {
            KeyCode::Char(c) => {
                self.confirmation_input.push(c);
                self.confirmation_error = None;
                InputResult::Continue
            }
            KeyCode::Backspace => {
                self.confirmation_input.pop();
                self.confirmation_error = None;
                InputResult::Continue
            }
            KeyCode::Enter => {
                if self.confirmation_input.trim() == "FLASH" {
                    self.confirmation_error = None;
                    self.kickoff_installation_process()
                } else {
                    self.confirmation_error =
                        Some("Type exactly: FLASH (case sensitive)".to_string());
                    InputResult::Continue
                }
            }
            KeyCode::Esc => {
                if let Some(prev) = self.current_step.prev() {
                    self.current_step = prev;
                }
                self.confirmation_input.clear();
                self.confirmation_error = None;
                InputResult::Continue
            }
            _ => InputResult::Continue,
        }
    }

    /// Determine what to do next: download image, download UEFI, or start flashing
    fn kickoff_installation_process(&mut self) -> InputResult {
        // Check if we need to download Fedora image
        if self.image_source_selection == ImageSource::DownloadFedora
            && self.downloaded_image_path.is_none()
        {
            self.current_step = InstallStep::DownloadingFedora;
            self.download_state = DownloadState::default();
            let version = ImageVersionOption::all()[self.selected_image_version_index]
                .version_str()
                .to_string();
            let edition = ImageEditionOption::all()[self.selected_image_edition_index]
                .edition_str()
                .to_string();
            let dest_dir = self.mash_root.join("downloads").join("images");
            return InputResult::StartDownload(DownloadType::FedoraImage {
                version,
                edition,
                dest_dir,
            });
        }

        // Check if we need to download UEFI firmware
        if self.download_uefi_firmware && self.downloaded_uefi_path.is_none() {
            self.current_step = InstallStep::DownloadingUefi;
            self.download_state = DownloadState::default();
            let dest_dir = self.mash_root.join("downloads").join("uefi");
            return InputResult::StartDownload(DownloadType::UefiFirmware { dest_dir });
        }

        // All downloads complete (or not needed), start flashing
        self.current_step = InstallStep::Flashing;
        if let Some(config) = self.build_flash_config() {
            InputResult::StartFlash(config)
        } else {
            self.confirmation_error = Some("Failed to build flash configuration".to_string());
            self.current_step = InstallStep::Confirmation;
            InputResult::Continue
        }
    }

    /// Build flash configuration from current app state
    fn build_flash_config(&self) -> Option<FlashConfig> {
        let disk = self
            .available_disks
            .get(self.selected_disk_index)
            .map(|d| d.path.clone())?;

        let locale = self
            .available_locales
            .get(self.selected_locale_index)
            .cloned();

        // Use downloaded paths if available, otherwise use user input
        let image_path = self
            .downloaded_image_path
            .clone()
            .unwrap_or_else(|| PathBuf::from(self.image_input.value()));

        let uefi_path = self
            .downloaded_uefi_path
            .clone()
            .unwrap_or_else(|| PathBuf::from(self.uefi_input.value()));

        Some(FlashConfig {
            image: image_path,
            disk,
            scheme: self.partition_plan.scheme,
            uefi_dir: uefi_path,
            dry_run: self.options.dry_run || self.dry_run_cli,
            auto_unmount: self.options.auto_unmount,
            watch: self.watch,
            locale,
            early_ssh: self.options.early_ssh,
            progress_tx: None,
            efi_size: self.partition_plan.efi_size.to_parted_string(),
            boot_size: self.partition_plan.boot_size.to_parted_string(),
            root_end: self.partition_plan.root_end.to_parted_string(),
            download_uefi_firmware: self.download_uefi_firmware,
            image_source_selection: self.image_source_selection,
            image_version: ImageVersionOption::all()[self.selected_image_version_index]
                .version_str()
                .to_string(),
            image_edition: ImageEditionOption::all()[self.selected_image_edition_index]
                .edition_str()
                .to_string(),
        })
    }

    /// Handler for downloading steps (both Fedora and UEFI)
    fn handle_download_input(&mut self, key: KeyEvent) -> InputResult {
        // During download, allow viewing progress or exiting on Esc/q/Ctrl+C
        match key.code {
            KeyCode::Enter | KeyCode::Esc | KeyCode::Char('q') => {
                if self.download_state.phase == DownloadPhase::Complete {
                    // Download finished, proceed to next step
                    self.kickoff_installation_process() // Check for next download or flash
                } else if self.download_state.phase == DownloadPhase::Failed {
                    // Download failed, allow going back to confirmation
                    self.current_step = InstallStep::Confirmation;
                    self.confirmation_input.clear();
                    self.download_state = DownloadState::default(); // Reset download state
                    InputResult::Continue
                } else {
                    // Download in progress, ignore input except global quit
                    InputResult::Continue
                }
            }
            _ => InputResult::Continue,
        }
    }

    fn handle_flashing_input(&mut self, key: KeyEvent) -> InputResult {
        // During flashing, only allow Ctrl+C (handled globally) to abort
        // Once flashing is complete, allow moving to the Complete screen.
        if self.progress.is_complete {
            match key.code {
                KeyCode::Enter => {
                    self.current_step = InstallStep::Complete;
                    InputResult::Continue
                }
                _ => InputResult::Continue,
            }
        } else {
            InputResult::Continue
        }
    }

    fn handle_complete_input(&mut self, key: KeyEvent) -> InputResult {
        match key.code {
            KeyCode::Enter | KeyCode::Esc | KeyCode::Char('q') => InputResult::Complete,
            _ => InputResult::Continue,
        }
    }

    /// Update progress state from channel
    pub fn update_progress(&mut self) {
        if let Some(ref rx) = self.progress_rx {
            while let Ok(update) = rx.try_recv() {
                self.progress.apply_update(update);
                if self.progress.is_complete {
                    self.install_success = self.progress.error.is_none();
                    self.install_error = self.progress.error.clone();
                }
            }
        }
    }

    /// Update download state from channel
    pub fn update_download(&mut self) -> InputResult {
        if let Some(ref rx) = self.download_rx {
            while let Ok(update) = rx.try_recv() {
                match update {
                    DownloadUpdate::Started {
                        description,
                        total_bytes,
                    } => {
                        self.download_state.is_downloading = true;
                        self.download_state.description = description;
                        self.download_state.total_bytes = total_bytes;
                        self.download_state.phase = DownloadPhase::Downloading;
                    }
                    DownloadUpdate::Progress {
                        current_bytes,
                        speed,
                        eta,
                    } => {
                        self.download_state.current_bytes = current_bytes;
                        self.download_state.speed_bytes_per_sec = speed;
                        self.download_state.eta_seconds = eta;
                    }
                    DownloadUpdate::Extracting => {
                        self.download_state.phase = DownloadPhase::Extracting;
                    }
                    DownloadUpdate::Complete(path) => {
                        self.download_state.is_downloading = false;
                        self.download_state.is_complete = true;
                        self.download_state.phase = DownloadPhase::Complete;

                        match self.current_step {
                            InstallStep::DownloadingFedora => {
                                self.downloaded_image_path = Some(path)
                            }
                            InstallStep::DownloadingUefi => self.downloaded_uefi_path = Some(path),
                            _ => {} // Should not happen
                        }

                        // Downloads finished, trigger next stage of installation
                        return self.kickoff_installation_process();
                    }
                    DownloadUpdate::Error(err) => {
                        self.download_state.is_downloading = false;
                        self.download_state.error = Some(err);
                        self.download_state.phase = DownloadPhase::Failed;
                    }
                }
            }
        }
        InputResult::Continue
    }

    /// Set up download channel and return sender
    pub fn setup_download_channel(&mut self) -> Sender<DownloadUpdate> {
        let (tx, rx) = mpsc::channel();
        self.download_rx = Some(rx);
        self.download_state = DownloadState::default();
        tx
    }

    /// Get the flash configuration if wizard completed
    pub fn get_flash_config(&self) -> Option<FlashConfig> {
        // If not in flashing or complete state, config is not ready
        if self.current_step != InstallStep::Flashing && self.current_step != InstallStep::Complete
        {
            return None;
        }

        self.build_flash_config()
    }

    /// Get selected disk info
    pub fn selected_disk(&self) -> Option<&DiskInfo> {
        self.available_disks.get(self.selected_disk_index)
    }

    /// Get selected locale
    pub fn selected_locale(&self) -> Option<&LocaleConfig> {
        self.available_locales.get(self.selected_locale_index)
    }
}
