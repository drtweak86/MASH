//! Application state machine for the TUI wizard

use crate::cli::Cli;
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
    ImageSelection,
    UefiDirectory,
    LocaleSelection,
    Options,
    Confirmation,
    Progress,
    Complete,
}

impl Screen {
    pub fn title(&self) -> &'static str {
        match self {
            Screen::Welcome => "Welcome",
            Screen::DiskSelection => "Select Target Disk",
            Screen::ImageSelection => "Select Image File",
            Screen::UefiDirectory => "UEFI Directory",
            Screen::LocaleSelection => "Locale & Keymap",
            Screen::Options => "Installation Options",
            Screen::Confirmation => "Confirm Installation",
            Screen::Progress => "Installing...",
            Screen::Complete => "Installation Complete",
        }
    }

    pub fn next(&self) -> Option<Screen> {
        match self {
            Screen::Welcome => Some(Screen::DiskSelection),
            Screen::DiskSelection => Some(Screen::ImageSelection),
            Screen::ImageSelection => Some(Screen::UefiDirectory),
            Screen::UefiDirectory => Some(Screen::LocaleSelection),
            Screen::LocaleSelection => Some(Screen::Options),
            Screen::Options => Some(Screen::Confirmation),
            Screen::Confirmation => Some(Screen::Progress),
            Screen::Progress => Some(Screen::Complete),
            Screen::Complete => None,
        }
    }

    pub fn prev(&self) -> Option<Screen> {
        match self {
            Screen::Welcome => None,
            Screen::DiskSelection => Some(Screen::Welcome),
            Screen::ImageSelection => Some(Screen::DiskSelection),
            Screen::UefiDirectory => Some(Screen::ImageSelection),
            Screen::LocaleSelection => Some(Screen::UefiDirectory),
            Screen::Options => Some(Screen::LocaleSelection),
            Screen::Confirmation => Some(Screen::Options),
            Screen::Progress => None, // Can't go back during progress
            Screen::Complete => None,
        }
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
    pub dry_run: bool,
}

impl Default for InstallOptions {
    fn default() -> Self {
        Self {
            auto_unmount: true,
            early_ssh: true, // Default ON as per spec
            dry_run: false,
        }
    }
}

/// Flash configuration collected from the wizard
#[derive(Debug, Clone)]
pub struct FlashConfig {
    pub image: PathBuf,
    pub disk: String,
    pub uefi_dir: PathBuf,
    pub dry_run: bool,
    pub auto_unmount: bool,
    pub watch: bool,
    pub locale: LocaleConfig,
    pub early_ssh: bool,
    pub progress_tx: Option<Sender<super::progress::ProgressUpdate>>,
}

/// Application state
pub struct App {
    pub current_screen: Screen,
    pub mash_root: PathBuf,
    pub watch: bool,
    pub dry_run_cli: bool, // From CLI flag

    // Disk selection
    pub available_disks: Vec<DiskInfo>,
    pub selected_disk_index: usize,

    // Image selection
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

    // Progress
    pub progress: ProgressState,
    pub progress_rx: Option<Receiver<super::progress::ProgressUpdate>>,

    // Complete
    pub install_success: bool,
    pub install_error: Option<String>,
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

            available_disks,
            selected_disk_index: 0,

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

            progress: ProgressState::default(),
            progress_rx: None,

            install_success: false,
            install_error: None,
        }
    }

    /// Handle keyboard input, returns action to take
    pub fn handle_input(&mut self, key: KeyEvent) -> InputResult {
        match self.current_screen {
            Screen::Welcome => self.handle_welcome_input(key),
            Screen::DiskSelection => self.handle_disk_selection_input(key),
            Screen::ImageSelection => self.handle_image_selection_input(key),
            Screen::UefiDirectory => self.handle_uefi_input(key),
            Screen::LocaleSelection => self.handle_locale_selection_input(key),
            Screen::Options => self.handle_options_input(key),
            Screen::Confirmation => self.handle_confirmation_input(key),
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
                    self.current_screen = Screen::ImageSelection;
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

    fn handle_image_selection_input(&mut self, key: KeyEvent) -> InputResult {
        // Handle text input
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
                        self.image_error = Some("Please select a .raw file, not a directory".into());
                    } else {
                        self.image_error = Some(format!("File not found: {}", path.display()));
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
                if self.options_focus < 2 {
                    self.options_focus += 1;
                }
                InputResult::Continue
            }
            KeyCode::Char(' ') | KeyCode::Enter => {
                // Toggle the focused option
                match self.options_focus {
                    0 => self.options.auto_unmount = !self.options.auto_unmount,
                    1 => self.options.early_ssh = !self.options.early_ssh,
                    2 => self.options.dry_run = !self.options.dry_run,
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
                    // Start installation
                    self.start_installation();
                    self.current_screen = Screen::Progress;
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

        Some(FlashConfig {
            image: PathBuf::from(self.image_input.value()),
            disk,
            uefi_dir: PathBuf::from(self.uefi_input.value()),
            dry_run: self.options.dry_run || self.dry_run_cli,
            auto_unmount: self.options.auto_unmount,
            watch: self.watch,
            locale,
            early_ssh: self.options.early_ssh,
            progress_tx: None, // Will be set up by the caller
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
