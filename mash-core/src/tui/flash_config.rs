use crate::cli::PartitionScheme;
use crate::locale::LocaleConfig;
use std::path::PathBuf;
use std::sync::mpsc::Sender;

use super::progress::ProgressUpdate;

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

/// Options for EFI image source selection (for Pi UEFI boot).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EfiSource {
    DownloadEfiImage,
    LocalEfiImage,
}

impl EfiSource {
    pub fn display(&self) -> &'static str {
        match self {
            EfiSource::DownloadEfiImage => "Download EFI image",
            EfiSource::LocalEfiImage => "Use local EFI image",
        }
    }

    pub fn all() -> &'static [EfiSource] {
        &[EfiSource::DownloadEfiImage, EfiSource::LocalEfiImage]
    }
}

/// OS Distribution options
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OsDistro {
    Fedora,
    Ubuntu,
    RaspberryPiOS,
    Manjaro,
}

impl OsDistro {
    pub fn display(&self) -> &'static str {
        match self {
            OsDistro::Fedora => "Fedora KDE (recommended)",
            OsDistro::Ubuntu => "Ubuntu Desktop (coming soon)",
            OsDistro::RaspberryPiOS => "Raspberry Pi OS (coming soon)",
            OsDistro::Manjaro => "Manjaro (coming soon)",
        }
    }

    pub fn is_available(&self) -> bool {
        matches!(self, OsDistro::Fedora)
    }

    pub fn all() -> &'static [OsDistro] {
        &[
            OsDistro::Fedora,
            OsDistro::Ubuntu,
            OsDistro::RaspberryPiOS,
            OsDistro::Manjaro,
        ]
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

/// Flash configuration collected from the Dojo UI
#[derive(Debug, Clone)]
pub struct TuiFlashConfig {
    pub image: PathBuf,
    pub disk: String,
    pub scheme: PartitionScheme,
    pub uefi_dir: PathBuf,
    pub dry_run: bool,
    pub auto_unmount: bool,
    pub watch: bool,
    pub locale: Option<LocaleConfig>,
    pub early_ssh: bool,
    pub progress_tx: Option<Sender<ProgressUpdate>>,
    pub efi_size: String,
    pub boot_size: String,
    pub root_end: String,
    pub download_uefi_firmware: bool, // New field to indicate UEFI firmware download
    pub image_source_selection: ImageSource, // New field to indicate image source
    pub image_version: String,        // New field for selected Fedora version
    pub image_edition: String,        // New field for selected Fedora edition
}

impl TryFrom<TuiFlashConfig> for crate::flash::FlashConfig {
    type Error = anyhow::Error;

    fn try_from(cfg: TuiFlashConfig) -> std::result::Result<Self, Self::Error> {
        let flash = crate::flash::FlashConfig {
            image: cfg.image,
            disk: cfg.disk,
            scheme: cfg.scheme,
            uefi_dir: cfg.uefi_dir,
            dry_run: cfg.dry_run,
            auto_unmount: cfg.auto_unmount,
            locale: cfg.locale,
            early_ssh: cfg.early_ssh,
            progress_tx: cfg.progress_tx,
            efi_size: cfg.efi_size,
            boot_size: cfg.boot_size,
            root_end: cfg.root_end,
        };
        flash.validate()?;
        Ok(flash)
    }
}
