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
    A friendly Dojo UI (TUI) for installing Fedora KDE on Raspberry Pi 4 with UEFI boot.\n\n\
    Run without arguments to launch the interactive Dojo UI (recommended! 🎉)\n\
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

    /// Developer mode - allows selecting the source/boot disk (dangerous; for debugging only)
    #[arg(long, global = true)]
    pub developer_mode: bool,

    /// Write logs to a file (TUI-safe; defaults to stderr when unavailable)
    #[arg(long, global = true)]
    pub log_file: Option<PathBuf>,
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

    /// 🧪 Run unified installer pipeline (dry-run by default)
    Install {
        /// Persisted state file path
        #[arg(long, default_value = "/var/lib/mash/state.json")]
        state: PathBuf,

        /// Enable dry-run mode (no changes)
        #[arg(long)]
        dry_run: bool,

        /// Execute plan (requires --confirm)
        #[arg(long)]
        execute: bool,

        /// Confirm destructive actions
        #[arg(long)]
        confirm: bool,

        /// Target disk device (for planning)
        #[arg(long)]
        disk: Option<String>,

        /// Mount spec: device:mountpoint[:fstype]
        #[arg(long)]
        mount: Vec<String>,

        /// ext4 format target device
        #[arg(long)]
        format_ext4: Vec<String>,

        /// btrfs format target device
        #[arg(long)]
        format_btrfs: Vec<String>,

        /// Packages to install
        #[arg(long)]
        package: Vec<String>,

        /// Include kernel USB-root fix in plan
        #[arg(long)]
        kernel_fix: bool,

        /// Root path for kernel USB-root fix
        #[arg(long)]
        kernel_fix_root: Option<PathBuf>,

        /// Path to mountinfo for kernel USB-root fix
        #[arg(long)]
        mountinfo_path: Option<PathBuf>,

        /// Path to /dev/disk/by-uuid for kernel USB-root fix
        #[arg(long)]
        by_uuid_path: Option<PathBuf>,

        /// Expected reboot count
        #[arg(long, default_value_t = 1)]
        reboots: u32,

        /// Mirror override for Fedora downloads
        #[arg(long)]
        download_mirror: Option<String>,

        /// Inline checksum (SHA256) for the download override
        #[arg(long)]
        download_checksum: Option<String>,

        /// URL to fetch the checksum from
        #[arg(long)]
        download_checksum_url: Option<String>,

        /// Timeout for download HTTP operations (seconds)
        #[arg(long, default_value_t = 120)]
        download_timeout_secs: u64,

        /// Number of retries for download attempts
        #[arg(long, default_value_t = 3)]
        download_retries: usize,

        /// Directory (relative to mash root) for downloaded assets
        #[arg(long, default_value = "downloads/images")]
        download_dir: PathBuf,
    },
}
