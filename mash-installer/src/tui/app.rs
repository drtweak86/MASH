//! Application state machine for the TUI wizard

use crate::cli::{Cli, PartitionScheme};
use crate::locale::LocaleConfig;
use crossterm::event::{KeyCode, KeyEvent};
use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, Sender};

use super::input::{InputField, InputMode};
use super::progress::ProgressState;
use super::widgets::DiskInfo;

/// Available screens in the wizard
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Screen {
    Welcome,
    DiskSelection,
    DownloadSourceSelection,
    ImageSelection,
    UefiDirectory,
    LocaleSelection,
    Options,
    Confirmation,
    DownloadingFedora, // New: downloading Fedora image
    DownloadingUefi,   // New: downloading UEFI firmware
    Progress,
    Complete,
}

impl Screen {
    pub fn title(&self) -> &'static str {
        match self {
            Screen::Welcome => "🥋 Enter the Dojo",
            Screen::DiskSelection => "💾 Select Target Disk",
            Screen::DownloadSourceSelection => "⬇️ Select Image Source",
            Screen::ImageSelection => "📀 Select Image File",
            Screen::UefiDirectory => "🔧 UEFI Configuration",
            Screen::LocaleSelection => "🌍 Locale & Keymap",
            Screen::Options => "⚙️ Installation Options",
            Screen::Confirmation => "⚠️ DANGER ZONE ⚠️",
            Screen::DownloadingFedora => "📥 Downloading Fedora Image",
            Screen::DownloadingUefi => "📥 Downloading UEFI Firmware",
            Screen::Progress => "🔥 Installing...",
            Screen::Complete => "🎉 Installation Complete!",
        }
    }

    pub fn next(&self) -> Option<Screen> {
        match self {
            Screen::Welcome => Some(Screen::DiskSelection),
            Screen::DiskSelection => Some(Screen::DownloadSourceSelection),
            Screen::DownloadSourceSelection => Some(Screen::ImageSelection),
            Screen::ImageSelection => Some(Screen::UefiDirectory),
            Screen::UefiDirectory => Some(Screen::LocaleSelection),
            Screen::LocaleSelection => Some(Screen::Options),
            Screen::Options => Some(Screen::Confirmation),
            // After confirmation, downloads happen (handled dynamically)
            Screen::Confirmation => Some(Screen::DownloadingFedora),
            Screen::DownloadingFedora => Some(Screen::DownloadingUefi),
            Screen::DownloadingUefi => Some(Screen::Progress),
            Screen::Progress => Some(Screen::Complete),
            Screen::Complete => None,
        }
    }

    pub fn prev(&self) -> Option<Screen> {
        match self {
            Screen::Welcome => None,
            Screen::DiskSelection => Some(Screen::Welcome),
            Screen::DownloadSourceSelection => Some(Screen::DiskSelection),
            Screen::ImageSelection => Some(Screen::DownloadSourceSelection),
            Screen::UefiDirectory => Some(Screen::ImageSelection),
            Screen::LocaleSelection => Some(Screen::UefiDirectory),
            Screen::Options => Some(Screen::LocaleSelection),
            Screen::Confirmation => Some(Screen::Options),
            Screen::DownloadingFedora => None, // Can't go back during download
            Screen::DownloadingUefi => None,   // Can't go back during download
            Screen::Progress => None,          // Can't go back during progress
            Screen::Complete => None,
        }
    }
}

/// Options for image source selection
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

/// Available Fedora image versions for download
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImageVersionOption {
    F43,
    F42,
    // Add more versions as needed
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

/// Available Fedora image editions for download (ARM aarch64)
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

/// Result of handling input
pub enum InputResult {
    Continue,
    Quit,
    Complete,
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
    Complete,
    Error(String),
}

/// Flash configuration collected from the wizard
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
    pub progress_tx: Option<Sender<super::progress::ProgressUpdate>>,
    pub efi_size: String,
    pub boot_size: String,
    pub root_end: String,
    pub download_uefi_firmware: bool, // New field to indicate UEFI firmware download
    pub image_source_selection: ImageSource, // New field to indicate image source
    pub image_version: String,        // New field for selected Fedora version
    pub image_edition: String,        // New field for selected Fedora edition
}

/// Application state
pub struct App {
    pub current_screen: Screen,
    pub mash_root: PathBuf,
    pub watch: bool,
    pub dry_run_cli: bool, // From CLI flag

    // Animation state
    pub animation_tick: u64, // Increments each frame for spinners/effects

    // Disk selection
    pub available_disks: Vec<DiskInfo>,
    pub selected_disk_index: usize,

    // Image source selection
    pub image_source_selection: ImageSource,
    pub selected_image_source_index: usize, // 0 for LocalFile, 1 for DownloadFedora
    pub selected_image_version_index: usize,
    pub selected_image_edition_index: usize,
    pub download_uefi_firmware: bool, // Option to download UEFI firmware

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
    pub options_focus: usize, // Which option is focused (0-2)

    // Confirmation
    pub confirmation_input: String,
    pub confirmation_error: Option<String>,

    // Download progress (for Fedora/UEFI download screens)
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

        Self {
            current_screen: Screen::Welcome,
            mash_root,
            watch,
            dry_run_cli: dry_run,

            animation_tick: 0,

            available_disks,
            selected_disk_index: 0,

            // New fields for image source selection
            image_source_selection: ImageSource::LocalFile,
            selected_image_source_index: 0,
            selected_image_version_index: 0, // Default to first version
            selected_image_edition_index: 0, // Default to first edition
            download_uefi_firmware: false,   // Default to not downloading UEFI firmware

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
            selected_locale_index: 0, // en_GB.UTF-8 is first

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

            efi_size_cli: match &cli.command {
                Some(crate::cli::Command::Flash { efi_size, .. }) => efi_size.clone(),
                _ => "1024MiB".to_string(),
            },
            boot_size_cli: match &cli.command {
                Some(crate::cli::Command::Flash { boot_size, .. }) => boot_size.clone(),
                _ => "2048MiB".to_string(),
            },
            root_end_cli: match &cli.command {
                Some(crate::cli::Command::Flash { root_end, .. }) => root_end.clone(),
                _ => "1800GiB".to_string(),
            },
        }
    }

    /// Handle keyboard input, returns action to take
    pub fn handle_input(&mut self, key: KeyEvent) -> InputResult {
        match self.current_screen {
            Screen::Welcome => self.handle_welcome_input(key),
            Screen::DiskSelection => self.handle_disk_selection_input(key),
            Screen::DownloadSourceSelection => self.handle_download_source_selection_input(key),
            Screen::ImageSelection => self.handle_image_selection_input(key),
            Screen::UefiDirectory => self.handle_uefi_input(key),
            Screen::LocaleSelection => self.handle_locale_selection_input(key),
            Screen::Options => self.handle_options_input(key),
            Screen::Confirmation => self.handle_confirmation_input(key),
            Screen::DownloadingFedora => self.handle_downloading_input(key),
            Screen::DownloadingUefi => self.handle_downloading_input(key),
            Screen::Progress => self.handle_progress_input(key),
            Screen::Complete => self.handle_complete_input(key),
        }
    }

    fn handle_welcome_input(&mut self, key: KeyEvent) -> InputResult {
        match key.code {
            KeyCode::Enter => {
                self.current_screen = Screen::DiskSelection;
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
                    self.current_screen = Screen::DownloadSourceSelection; // Changed next screen
                }
                InputResult::Continue
            }
            KeyCode::Esc => {
                if let Some(prev) = self.current_screen.prev() {
                    self.current_screen = prev;
                }
                InputResult::Continue
            }
            KeyCode::Char('r') => {
                // Refresh disk list
                self.available_disks = DiskInfo::scan_disks();
                self.selected_disk_index = 0;
                InputResult::Continue
            }
            _ => InputResult::Continue,
        }
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
                    _ => unreachable!(), // Should not happen with max_source_index check
                };
                self.current_screen = Screen::ImageSelection;
                InputResult::Continue
            }
            KeyCode::Esc => {
                if let Some(prev) = self.current_screen.prev() {
                    self.current_screen = prev;
                }
                InputResult::Continue
            }
            _ => InputResult::Continue,
        }
    }

    fn handle_image_selection_input(&mut self, key: KeyEvent) -> InputResult {
        // This screen now behaves differently based on image_source_selection
        match self.image_source_selection {
            ImageSource::LocalFile => {
                // Existing logic for local file input
                if self.image_input.mode == InputMode::Editing {
                    match key.code {
                        KeyCode::Enter => {
                            // Validate path
                            let path = PathBuf::from(self.image_input.value());
                            if path.exists() && path.is_file() {
                                self.image_error = None;
                                self.image_input.mode = InputMode::Normal;
                                self.current_screen = Screen::UefiDirectory;
                            } else if path.is_dir() {
                                // If it's a directory, look for .raw files
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
                            if let Some(prev) = self.current_screen.prev() {
                                self.current_screen = prev;
                            }
                            InputResult::Continue
                        }
                        KeyCode::Tab => {
                            // Skip validation and move to next screen
                            self.current_screen = Screen::UefiDirectory;
                            InputResult::Continue
                        }
                        _ => InputResult::Continue,
                    }
                }
            }
            ImageSource::DownloadFedora => {
                // Logic for selecting Fedora version/edition for download
                let max_version_index = ImageVersionOption::all().len().saturating_sub(1);
                let max_edition_index = ImageEditionOption::all().len().saturating_sub(1);

                match key.code {
                    KeyCode::Up | KeyCode::Char('k') => {
                        if self.options_focus == 0 {
                            // Navigating versions
                            if self.selected_image_version_index > 0 {
                                self.selected_image_version_index -= 1;
                            }
                        } else {
                            // Navigating editions
                            if self.selected_image_edition_index > 0 {
                                self.selected_image_edition_index -= 1;
                            }
                        }
                        InputResult::Continue
                    }
                    KeyCode::Down | KeyCode::Char('j') => {
                        if self.options_focus == 0 {
                            // Navigating versions
                            if self.selected_image_version_index < max_version_index {
                                self.selected_image_version_index += 1;
                            }
                        } else {
                            // Navigating editions
                            if self.selected_image_edition_index < max_edition_index {
                                self.selected_image_edition_index += 1;
                            }
                        }
                        InputResult::Continue
                    }
                    KeyCode::Left | KeyCode::Right => {
                        // Toggle focus between version and edition selection
                        self.options_focus = (self.options_focus + 1) % 2;
                        InputResult::Continue
                    }
                    KeyCode::Enter | KeyCode::Tab => {
                        // Proceed to UEFI Directory screen
                        self.current_screen = Screen::UefiDirectory;
                        InputResult::Continue
                    }
                    KeyCode::Esc => {
                        if let Some(prev) = self.current_screen.prev() {
                            self.current_screen = prev;
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
                        self.current_screen = Screen::LocaleSelection;
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
                    // New: Toggle download UEFI firmware
                    self.download_uefi_firmware = !self.download_uefi_firmware;
                    InputResult::Continue
                }
                KeyCode::Enter | KeyCode::Char('e') | KeyCode::Char('i') => {
                    self.uefi_input.mode = InputMode::Editing;
                    InputResult::Continue
                }
                KeyCode::Esc => {
                    if let Some(prev) = self.current_screen.prev() {
                        self.current_screen = prev;
                    }
                    InputResult::Continue
                }
                KeyCode::Tab => {
                    self.current_screen = Screen::LocaleSelection;
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
                self.current_screen = Screen::Options;
                InputResult::Continue
            }
            KeyCode::Esc => {
                if let Some(prev) = self.current_screen.prev() {
                    self.current_screen = prev;
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
                // Toggle the focused option
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
                self.current_screen = Screen::Confirmation;
                InputResult::Continue
            }
            KeyCode::Esc => {
                if let Some(prev) = self.current_screen.prev() {
                    self.current_screen = prev;
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
                if self.confirmation_input.trim() == "YES I KNOW" {
                    // Determine next screen based on what needs to be downloaded
                    self.current_screen = self.next_screen_after_confirmation();
                } else {
                    self.confirmation_error =
                        Some("Type exactly: YES I KNOW (case sensitive)".into());
                }
                InputResult::Continue
            }
            KeyCode::Esc => {
                if let Some(prev) = self.current_screen.prev() {
                    self.current_screen = prev;
                }
                self.confirmation_input.clear();
                self.confirmation_error = None;
                InputResult::Continue
            }
            _ => InputResult::Continue,
        }
    }

    /// Determine next screen after confirmation based on download selections
    fn next_screen_after_confirmation(&mut self) -> Screen {
        if self.image_source_selection == ImageSource::DownloadFedora {
            // Need to download Fedora image
            self.download_state = DownloadState::default();
            Screen::DownloadingFedora
        } else if self.download_uefi_firmware {
            // Only need to download UEFI
            self.download_state = DownloadState::default();
            Screen::DownloadingUefi
        } else {
            // No downloads needed, go straight to flashing
            self.start_installation();
            Screen::Progress
        }
    }

    /// Determine next screen after Fedora download
    pub fn next_screen_after_fedora_download(&mut self) -> Screen {
        if self.download_uefi_firmware {
            self.download_state = DownloadState::default();
            Screen::DownloadingUefi
        } else {
            self.start_installation();
            Screen::Progress
        }
    }

    /// Handler for download screens (both Fedora and UEFI)
    fn handle_downloading_input(&mut self, key: KeyEvent) -> InputResult {
        // During download, only allow viewing progress
        // Ctrl+C is handled globally to abort
        if self.download_state.phase == DownloadPhase::Complete {
            match key.code {
                KeyCode::Enter => {
                    // Move to next screen
                    if self.current_screen == Screen::DownloadingFedora {
                        self.current_screen = self.next_screen_after_fedora_download();
                    } else {
                        // After UEFI download, start installation
                        self.start_installation();
                        self.current_screen = Screen::Progress;
                    }
                    InputResult::Continue
                }
                _ => InputResult::Continue,
            }
        } else if self.download_state.phase == DownloadPhase::Failed {
            match key.code {
                KeyCode::Enter | KeyCode::Esc => {
                    // Go back to confirmation to retry
                    self.current_screen = Screen::Confirmation;
                    self.confirmation_input.clear();
                    InputResult::Continue
                }
                _ => InputResult::Continue,
            }
        } else {
            InputResult::Continue
        }
    }

    fn handle_progress_input(&mut self, key: KeyEvent) -> InputResult {
        // During progress, only allow Ctrl+C (handled globally) to abort
        // Check if installation is complete
        if self.progress.is_complete {
            match key.code {
                KeyCode::Enter => {
                    self.current_screen = Screen::Complete;
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

    fn start_installation(&mut self) {
        // Set up progress channel
        let (_tx, rx) = mpsc::channel();
        self.progress_rx = Some(rx);
        self.progress = ProgressState::default();

        // Store the sender for flash::run_with_progress
        // The actual flash will be started after we return from the TUI loop
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
    pub fn update_download(&mut self) {
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
                    DownloadUpdate::Complete => {
                        self.download_state.is_downloading = false;
                        self.download_state.is_complete = true;
                        self.download_state.phase = DownloadPhase::Complete;
                    }
                    DownloadUpdate::Error(err) => {
                        self.download_state.is_downloading = false;
                        self.download_state.error = Some(err);
                        self.download_state.phase = DownloadPhase::Failed;
                    }
                }
            }
        }
    }

    /// Set up download channel and return sender
    pub fn setup_download_channel(&mut self) -> Sender<DownloadUpdate> {
        let (tx, rx) = mpsc::channel();
        self.download_rx = Some(rx);
        self.download_state = DownloadState::default();
        tx
    }

    /// Check if downloads are needed
    pub fn needs_fedora_download(&self) -> bool {
        self.image_source_selection == ImageSource::DownloadFedora
    }

    pub fn needs_uefi_download(&self) -> bool {
        self.download_uefi_firmware
    }

    /// Get the flash configuration if wizard completed
    pub fn get_flash_config(&self) -> Option<FlashConfig> {
        if self.current_screen != Screen::Progress && self.current_screen != Screen::Complete {
            return None;
        }

        let disk = self
            .available_disks
            .get(self.selected_disk_index)
            .map(|d| d.path.clone())?;

        let locale = self
            .available_locales
            .get(self.selected_locale_index)
            .cloned()?;

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
            scheme: self.options.partition_scheme,
            uefi_dir: uefi_path,
            dry_run: self.options.dry_run || self.dry_run_cli,
            auto_unmount: self.options.auto_unmount,
            watch: self.watch,
            locale: Some(locale),
            early_ssh: self.options.early_ssh,
            progress_tx: None, // Will be set up by the caller
            efi_size: self.efi_size_cli.clone(),
            boot_size: self.boot_size_cli.clone(),
            root_end: self.root_end_cli.clone(),
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

    /// Get selected disk info
    pub fn selected_disk(&self) -> Option<&DiskInfo> {
        self.available_disks.get(self.selected_disk_index)
    }

    /// Get selected locale
    pub fn selected_locale(&self) -> Option<&LocaleConfig> {
        self.available_locales.get(self.selected_locale_index)
    }
}
