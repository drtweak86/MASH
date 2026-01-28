use clap::{Parser, Subcommand};
use std::path::PathBuf;

/// 🥋 MASH Installer – Fedora KDE (RPi4 UEFI) flashing + Dojo staging
#[derive(Parser, Debug)]
#[command(name="mash-installer", version, about="🥋 MASH Installer", long_about=None)]
pub struct Cli {
    /// Override the MASH workspace root (default: ~/MASH)
    #[arg(long, value_name="MASH_ROOT", default_value_os_t=default_mash_root())]
    pub mash_root: PathBuf,

    #[command(subcommand)]
    pub cmd: Command,
}

#[derive(Subcommand, Debug)]
pub enum Command {
    /// Phase 1A: preflight checks (tools present, permissions, etc.)
    Preflight {
        /// Print what would happen, make no changes.
        #[arg(long)]
        dry_run: bool,
    },

    /// Phase 1B/1C: flash an image onto a target disk and stage Dojo
    Flash {
        /// Path to Fedora raw image (.raw)
        #[arg(long, value_name="IMAGE")]
        image: PathBuf,

        /// Target disk (e.g. /dev/sda). WARNING: this will be wiped.
        #[arg(long, value_name="DISK")]
        disk: String,

        /// Directory containing UEFI files (e.g. ~/MASH/uefi)
        #[arg(long, value_name="UEFI_DIR")]
        uefi_dir: PathBuf,

        /// Attempt to unmount anything mounted from the target disk (after confirmation)
        #[arg(long)]
        auto_unmount: bool,

        /// Print what would happen, make no changes.
        #[arg(long)]
        dry_run: bool,

        /// Live watch mode (rsync progress + disk activity)
        #[arg(long)]
        watch: bool,

        /// Skip the interactive confirmation gate (DANGEROUS)
        #[arg(long)]
        yes_i_know: bool,
    },

    /// Enter the Dojo (interactive wizard) – collects inputs then runs Flash
    Tui {
        /// Live watch mode (rsync progress + disk activity)
        #[arg(long)]
        watch: bool,

        /// Print what would happen, make no changes.
        #[arg(long)]
        dry_run: bool,
    },
}

fn default_mash_root() -> PathBuf {
    let home = std::env::var_os("HOME").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("/"));
    home.join("MASH")
}
