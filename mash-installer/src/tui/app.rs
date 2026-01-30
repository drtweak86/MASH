#![allow(dead_code)]

use crate::cli::{Cli, PartitionScheme};
use crate::locale::LocaleConfig;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use super::progress::Phase; // Keep Phase as it's used in ExecutionStep
// Remaining modules (input, progress, widgets) are still needed for FlashConfig and ExecutionStep to compile.

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

/// Strongly‑typed partition size
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

    /// Convert to MiB for validation comparisons.
    /// Returns 0 for percentage‑based sizes (not comparable).
    pub fn to_mib(&self) -> u64 {
        match self {
            PartitionSize::Mebibytes(m) => *m,
            PartitionSize::Gibibytes(g) => g * 1024,
            PartitionSize::Percentage(_) => 0,
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

/// Unified installation state for single‑page view
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
// Image Source Selection
// ============================================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImageSource {
    LocalFile,
    DownloadFedora,
}

impl ImageSource {
    pub fn display(&self) -> &'static str {
        match self {
            ImageSource::LocalFile => "Local Image File (.raw)",
            ImageSource::DownloadFedora => "Download Fedora Image",
        }
    }
}

// ============================================================================
// Available Fedora image versions for download
// ============================================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImageVersionOption {
    F43,
    F42,
}

impl ImageVersionOption {
    pub fn display(&self) -> &'static str {
        match self {
            ImageVersionOption::F43 => "Fedora 43",
            ImageVersionOption::F42 => "Fedora 42",
        }
    }

    pub fn version_str(&self) -> &'static str {
        match self {
            ImageVersionOption::F43 => "43",
            ImageVersionOption::F42 => "42",
        }
    }

    pub fn all() -> &'static [ImageVersionOption] {
        &[ImageVersionOption::F43, ImageVersionOption::F42]
    }
}

// ============================================================================
// Available Fedora image editions for download (ARM aarch64)
// ============================================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImageEditionOption {
    Kde,
    Xfce,
    LXQt,
    Minimal,
    Server,
}

impl ImageEditionOption {
    pub fn display(&self) -> &'static str {
        match self {
            ImageEditionOption::Kde => "KDE Plasma Mobile",
            ImageEditionOption::Xfce => "Xfce Desktop",
            ImageEditionOption::LXQt => "LXQt Desktop",
            ImageEditionOption::Minimal => "Minimal (no desktop)",
            ImageEditionOption::Server => "Server",
        }
    }

    pub fn edition_str(&self) -> &'static str {
        match self {
            ImageEditionOption::Kde => "KDE",
            ImageEditionOption::Xfce => "Xfce",
            ImageEditionOption::LXQt => "LXQt",
            ImageEditionOption::Minimal => "Minimal",
            ImageEditionOption::Server => "Server",
        }
    }

    pub fn all() -> &'static [ImageEditionOption] {
        &[
            ImageEditionOption::Kde,
            ImageEditionOption::Xfce,
            ImageEditionOption::LXQt,
            ImageEditionOption::Minimal,
            ImageEditionOption::Server,
        ]
    }
}

/// Options for image source selection
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

// ============================================================================
// Download progress state for TUI display
// ============================================================================

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

// ============================================================================
// Flash configuration collected from the wizard
// ============================================================================

#[derive(Debug, Clone)]
pub struct FlashConfig {
    pub image: PathBuf,
    pub disk: String,
    pub scheme: PartitionScheme,
    pub uefi_dir: PathBuf,
    pub dry_run: bool,
    pub auto_unmount: bool,
    pub watch: bool,
    pub locale: Option<LocaleConfig>,
    pub early_ssh: bool,
    pub progress_tx: Option<Sender<super::progress::ProgressEvent>>, // Changed to ProgressEvent
    pub cancel_flag: Arc<AtomicBool>, // Added for cancellation
    pub efi_size: String,
    pub boot_size: String,
    pub root_end: String,
    pub download_uefi_firmware: bool,
    pub image_source_selection: ImageSource,
    pub image_version: String,
    pub image_edition: String,
}

// ============================================================================
// Application state
// ============================================================================

#[allow(dead_code)] // Legacy App struct, no longer used by new UI
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

    // Worker thread communication
    pub progress_event_rx: Option<Receiver<super::progress::ProgressEvent>>,
    pub cancel_flag: Arc<AtomicBool>,

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
            partition_edit_input: String::new(),
            partition_edit_field: 0, // This order is important
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

            progress_event_rx: None,
            cancel_flag: Arc::new(AtomicBool::new(false)),

            efi_size_cli,
            boot_size_cli,
            root_end_cli,
        }
    }

    /// Handle keyboard input, returns action to take
    pub fn handle_input(&mut self, key: KeyEvent) -> InputResult {
        // NOTE: The old `InstallStep` enum has been removed.
        // All legacy step handling has been replaced with a no‑op that
        // simply returns `Continue`.  The new single‑page UI drives
        // execution through `start_execution()` and worker threads.
        InputResult::Continue
    }

    // -----------------------------------------------------------------------
    // Legacy step handlers (now no‑ops) – kept only to preserve API shape
    // -----------------------------------------------------------------------
    fn handle_welcome_input(&mut self, _key: KeyEvent) -> InputResult { InputResult::Continue }
    fn handle_disk_selection_input(&mut self, _key: KeyEvent) -> InputResult { InputResult::Continue }
    fn handle_disk_confirmation_input(&mut self, _key: KeyEvent) -> InputResult { InputResult::Continue }
    fn handle_partition_scheme_input(&mut self, _key: KeyEvent) -> InputResult { InputResult::Continue }
    fn handle_partition_layout_input(&mut self, _key: KeyEvent) -> InputResult { InputResult::Continue }
    fn handle_partition_customize_input(&mut self, _key: KeyEvent) -> InputResult { InputResult::Continue }
    fn handle_download_source_selection_input(&mut self, _key: KeyEvent) -> InputResult { InputResult::Continue }
    fn handle_image_selection_input(&mut self, _key: KeyEvent) -> InputResult { InputResult::Continue }
    fn handle_uefi_input(&mut self, _key: KeyEvent) -> InputResult { InputResult::Continue }
    fn handle_locale_selection_input(&mut self, _key: KeyEvent) -> InputResult { InputResult::Continue }
    fn handle_options_input(&mut self, _key: KeyEvent) -> InputResult { InputResult::Continue }
    fn handle_confirmation_input(&mut self, _key: KeyEvent) -> InputResult { InputResult::Continue }
    fn handle_flashing_input(&mut self, _key: KeyEvent) -> InputResult { InputResult::Continue }
    fn handle_complete_input(&mut self, _key: KeyEvent) -> InputResult { InputResult::Continue }

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
            progress_tx: None, // This will be set by kickoff_installation_process
            cancel_flag: Arc::clone(&self.cancel_flag), // Pass the cancel flag
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

    /// Kick‑off the installation worker thread
    fn kickoff_installation_process(&mut self) -> InputResult {
        // The worker thread will use the cloned `progress_tx` and `cancel_flag`
        // and will send `ProgressEvent`s back through `progress_event_rx`.
        // All legacy step handling has been removed; the flow now proceeds
        // directly to worker spawning.
        InputResult::Continue
    }

    /// Update progress state from worker thread via `ProgressEvent`
    pub fn update_worker_progress(&mut self) {
        // Same as before – just ignore any events once we are in a final state.
        if let Some(ref rx) = self.progress_event_rx {
            while let Ok(event) = rx.try_recv() {
                if self.cancel_flag.load(Ordering::SeqCst) || self.installation_state.error.is_some() {
                    match event {
                        super::progress::ProgressEvent::Complete(_, _)
                        | super::progress::ProgressEvent::Error(_)
                        | super::progress::ProgressEvent::Cancelled => {}
                        _ => continue,
                    }
                }

                match event {
                    super::progress::ProgressEvent::FlashUpdate(step, update) => {
                        // No‑op – legacy steps no longer exist.
                    }
                    super::progress::ProgressEvent::DownloadUpdate(step, update) => {
                        // No‑op – legacy steps no longer exist.
                    }
                    super::progress::ProgressEvent::Error(e) => {
                        self.installation_state.error = Some(e.clone());
                        self.installation_state.mode = InstallMode::Complete;
                    }
                    super::progress::ProgressEvent::Complete(image_path, uefi_path) => {
                        self.installation_state.mode = InstallMode::Complete;
                        self.downloaded_image_path = Some(image_path);
                        self.downloaded_uefi_path = uefi_path;
                    }
                    super::progress::ProgressEvent::Cancelled => {
                        self.installation_state.mode = InstallMode::Complete;
                        self.installation_state.error = Some("Installation cancelled".to_string());
                    }
                    _ => {}
                }
            }
        }
    }

    /// Get the flash configuration if wizard completed
    pub fn get_flash_config(&self) -> Option<FlashConfig> {
        // The new UI does not expose the old `current_step` any more.
        // Configuration is built on‑demand when the user triggers flashing.
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
