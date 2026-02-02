//! CLI argument parsing for MASH
//!
//! Makes TUI the default entry point when no subcommand is provided.

use clap::{Parser, Subcommand, ValueEnum};
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum PartitionScheme {
    /// MBR (msdos) partition table — recommended for maximum Raspberry Pi UEFI compatibility
    Mbr,
    /// GPT partition table
    Gpt,
}

impl std::fmt::Display for PartitionScheme {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PartitionScheme::Mbr => write!(f, "MBR"),
            PartitionScheme::Gpt => write!(f, "GPT"),
        }
    }
}

#[derive(Parser)]
#[command(name = "mash")]
#[command(about = "🍠 MASH - Fedora KDE for Raspberry Pi 4B")]
#[command(long_about = "🍠 MASH - Fedora KDE for Raspberry Pi 4B\n\n\
    A friendly TUI wizard for installing Fedora KDE on Raspberry Pi 4 with UEFI boot.\n\n\
    Run without arguments to launch the interactive TUI wizard (recommended! 🎉)\n\
    Or use subcommands for CLI scripting.")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Command>,

    /// Run a specific installation stage and exit (e.g., 10_locale_uk)
    #[arg(short = 's', long, global = true)]
    pub stage: Option<String>,

    /// Arguments for the selected stage (repeatable)
    #[arg(long, global = true)]
    pub stage_arg: Vec<String>,

    /// Run in dry-run mode (no changes made)
    #[arg(long, global = true)]
    pub dry_run: bool,

    /// Watch mode - monitor filesystem after installation
    #[arg(long, global = true)]
    pub watch: bool,

    /// MASH root directory (contains images/ and uefi/ subdirs)
    #[arg(long, default_value = ".", global = true)]
    pub mash_root: PathBuf,

    /// Dump TUI step render text to stdout and exit
    #[arg(long, global = true)]
    pub dump_tui: bool,
}

#[derive(Subcommand)]
#[allow(clippy::large_enum_variant)]
pub enum Command {
    /// 🔍 Run preflight checks (verify system requirements)
    Preflight,

    /// 💾 Flash Fedora image to disk (CLI mode for scripting)
    Flash {
        /// Path to Fedora .raw image file (omit when using --download-image)
        #[arg(long, required_unless_present = "download_image")]
        image: Option<PathBuf>,

        /// Target disk device (e.g., /dev/sda)
        #[arg(long)]
        disk: String,

        /// Partition scheme (MBR recommended; GPT available)
        #[arg(long, value_enum, default_value = "mbr")]
        scheme: PartitionScheme,

        /// Directory containing UEFI files (omit when using --download-uefi)
        #[arg(long, required_unless_present = "download_uefi")]
        uefi_dir: Option<PathBuf>,

        /// Automatically unmount target disk partitions
        #[arg(long)]
        auto_unmount: bool,

        /// Confirm destructive operation (required for non-dry-run)
        #[arg(long)]
        yes_i_know: bool,

        /// Locale in format "lang:keymap" (e.g., "en_GB.UTF-8:uk")
        #[arg(long)]
        locale: Option<String>,

        /// Enable early SSH access before graphical login
        #[arg(long)]
        early_ssh: bool,

        /// EFI partition size (e.g., "1024MiB")
        #[arg(long, default_value = "1024MiB")]
        efi_size: String,

        /// BOOT partition size (e.g., "2048MiB")
        #[arg(long, default_value = "2048MiB")]
        boot_size: String,

        /// End of ROOT partition (e.g., "1800GiB"). DATA uses the rest.
        #[arg(long, default_value = "1800GiB")]
        root_end: String,

        /// Automatically download UEFI firmware from GitHub
        #[arg(long)]
        download_uefi: bool,

        /// Automatically download Fedora .raw.xz image
        #[arg(long)]
        download_image: bool,

        /// Fedora release version to download (e.g., "43")
        #[arg(long, default_value = "43")]
        image_version: String,

        /// Fedora edition to download (e.g., "KDE")
        #[arg(long, default_value = "KDE")]
        image_edition: String,
    },

    /// 🧭 Stage starship.toml into the assets directory
    StageStarshipToml {
        /// Path to staging directory
        #[arg(long)]
        stage_dir: PathBuf,

        /// Path to starship.toml
        #[arg(long)]
        starship_toml: PathBuf,
    },
}
