use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(name="mash-installer", version="0.1.0", about="🦀 MASH Installer 0.1")]
pub struct Cli {
    /// Override the MASH workspace root (default: ~/MASH)
    #[arg(long, default_value="~/MASH")]
    pub mash_root: String,

    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Phase 1A: Check that your workspace + tools look sane
    Preflight {
        /// Print checks but don't change anything
        #[arg(long)]
        dry_run: bool,
    },

    /// Phase 1B: Disk flash (safe scaffold)
    Flash {
        /// Fedora raw image path
        #[arg(long)]
        image: PathBuf,

        /// Target disk (e.g. "sda" or "/dev/sda")
        #[arg(long)]
        disk: String,

        /// Directory containing RPI_EFI.fd + UEFI zip (optional)
        #[arg(long, value_name="UEFI_DIR")]
        uefi_dir: PathBuf,

        /// Print plan but do not touch disk
        #[arg(long)]
        dry_run: bool,

        /// Attempt to unmount anything on the target disk (asks unless --yes-i-know)
        #[arg(long)]
        auto_unmount: bool,

        /// Required to perform destructive steps (and to auto-unmount without asking)
        #[arg(long)]
        yes_i_know: bool,
    },

    /// Phase 1C: Dojo (TUI) — coming soon
    Dojo {
        /// Force show even if already completed
        #[arg(long)]
        force: bool,
    }
}
