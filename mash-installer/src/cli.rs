use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "mash-installer")]
#[command(about = "MASH Installer - Fedora KDE for Raspberry Pi 4B")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand)]
pub enum Command {
    /// Run preflight checks
    Preflight {
        #[arg(long)]
        dry_run: bool,
    },
    /// Flash Fedora image to disk
    Flash {
        #[arg(long)]
        image: PathBuf,
        
        #[arg(long)]
        disk: String,
        
        #[arg(long)]
        uefi_dir: PathBuf,
        
        #[arg(long)]
        dry_run: bool,
        
        #[arg(long)]
        auto_unmount: bool,
        
        #[arg(long)]
        yes_i_know: bool,
    },
}
